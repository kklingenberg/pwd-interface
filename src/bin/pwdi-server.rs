use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::error::InternalError;
use actix_web::http::StatusCode;
use actix_web::web::{block, get, post, resource, Data};
use actix_web::{middleware, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web_httpauth::headers::www_authenticate;
use actix_web_httpauth::middleware::HttpAuthentication;
use anyhow::{anyhow, Result};
use clap::Parser;
use futures::TryStreamExt;
use once_cell::sync::OnceCell;
use pwd_interface::{bundler, token};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::NamedTempFile;

/// An HTTP interface to POST and GET a folder to and out of a PWD
/// instance.
#[derive(Parser, Clone)]
struct Settings {
    /// Bind the server to this port
    #[clap(long, short, env, default_value_t = 80)]
    port: u16,

    /// Serve this folder (or the current folder, if omitted)
    path: Option<PathBuf>,
}

/// Global token
static TOKEN: OnceCell<String> = OnceCell::new();

/// Global path to a directory to be served and replaced
static PATH: OnceCell<PathBuf> = OnceCell::new();
fn get_path() -> actix_web::Result<&'static Path> {
    PATH.get().map(|p| p.as_path()).ok_or_else(|| {
        InternalError::new("misconfigured server", StatusCode::INTERNAL_SERVER_ERROR).into()
    })
}

/// Handles a pull from a client
async fn handle_pull(
    req: HttpRequest,
    bundler: Data<Mutex<bundler::Bundler>>,
) -> actix_web::Result<HttpResponse> {
    let base_path = get_path()?;
    let bundle = block({
        let bundler = bundler.into_inner();
        move || bundler.lock().unwrap().make(base_path)
    })
    .await?
    .map_err(|e| InternalError::new(format!("{:?}", e), StatusCode::INTERNAL_SERVER_ERROR))?;
    NamedFile::open_async(bundle)
        .await
        .map(|f| f.into_response(&req))
        .map_err(|e| e.into())
}

/// Handle a push from a client
async fn handle_push(
    mut payload: Multipart,
    bundler: Data<Mutex<bundler::Bundler>>,
) -> actix_web::Result<HttpResponse> {
    if let Some(mut field) = payload.try_next().await? {
        let target_path = get_path()?;
        let file = NamedTempFile::new()?;
        let mut f = block({
            let file_path = file.path().to_path_buf();
            move || File::create(file_path.as_path())
        })
        .await??;
        while let Some(chunk) = field.try_next().await? {
            f = block(move || f.write_all(&chunk).map(|_| f)).await??;
        }
        block({
            let file_path = file.path().to_path_buf();
            let bundler = bundler.into_inner();
            move || {
                bundler
                    .lock()
                    .unwrap()
                    .extract(file_path.as_path(), target_path)
            }
        })
        .await?
        .map_err(|e| InternalError::new(format!("{:?}", e), StatusCode::INTERNAL_SERVER_ERROR))?;
        Ok(HttpResponse::Created().body("Pushed"))
    } else {
        Ok(HttpResponse::BadRequest().body("Expected a file in push message"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::parse();
    let bundler = Data::new(Mutex::new(bundler::Bundler::new()));
    let port = settings.port;
    let path = settings.path.unwrap_or(PathBuf::from("."));
    if !path.is_dir() {
        return Err(anyhow!(
            "PATH must point to an existing directory (given: {:?})",
            path
        ));
    }
    let session_token = token::make(21);
    println!("Token: {:?}", &session_token);

    TOKEN
        .set(session_token)
        .map_err(|_| anyhow!("couldn't set global token"))?;
    PATH.set(path)
        .map_err(|_| anyhow!("couldn't set global path"))?;

    HttpServer::new({
        let bundler = bundler.clone();
        move || {
            App::new()
                .wrap(middleware::Compress::default())
                .wrap(middleware::NormalizePath::trim())
                .wrap(HttpAuthentication::basic(|req, credentials| async move {
                    if matches!((
                    TOKEN.get(),
                    credentials.user_id()
                ), (
                    Some(expected_token),
                    given_token,
                ) if token::verify_timed(given_token, expected_token))
                    {
                        Ok(req)
                    } else {
                        Err((
                            AuthenticationError::new(www_authenticate::basic::Basic::with_realm(
                                "Restricted",
                            ))
                            .into(),
                            req,
                        ))
                    }
                }))
                .app_data(bundler.clone())
                .service(
                    resource("/")
                        .route(get().to(handle_pull))
                        .route(post().to(handle_push)),
                )
        }
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await?;
    Ok(())
}
