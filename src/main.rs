use actix_files as fs;
use actix_http::header::TryIntoHeaderValue;
use actix_multipart::Multipart;
use actix_web::web::{block, post, Data};
use actix_web::{middleware, App, HttpResponse, HttpServer};
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web_httpauth::headers::{authorization, www_authenticate};
use actix_web_httpauth::middleware::HttpAuthentication;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use futures::TryStreamExt;
use once_cell::sync::OnceCell;
use rand::{thread_rng, RngCore};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// An HTTP interface to POST and GET files to and out of a PWD
/// instance.
#[derive(Parser, Clone)]
struct Settings {
    /// Bind the server to this port
    #[clap(long, short, env, default_value_t = 80)]
    port: u16,

    #[clap(long, short, env)]
    base_path: Option<PathBuf>,
}

/// Generate a random token of at least the requested size, in bytes
fn random_token(size: usize) -> String {
    let fitted = if size % 3 == 0 {
        size
    } else {
        size + 3 - (size % 3)
    };
    let mut data: Vec<u8> = vec![0; fitted];
    thread_rng().fill_bytes(&mut data);
    general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Receives a file and persists it to the base path
async fn create_file(
    mut payload: Multipart,
    base_path: Data<PathBuf>,
) -> actix_web::Result<HttpResponse> {
    let mut file_names: Vec<(Option<String>, String)> = Vec::new();
    while let Some(mut field) = payload.try_next().await? {
        let file_name = random_token(21);
        let file_path = base_path.join(&file_name);
        let field_name = field.content_disposition().get_filename().map(String::from);
        let mut f = block(|| File::create(file_path)).await??;
        while let Some(chunk) = field.try_next().await? {
            f = block(move || f.write_all(&chunk).map(|_| f)).await??;
        }
        file_names.push((field_name, file_name))
    }
    Ok(HttpResponse::Created().body(
        file_names
            .into_iter()
            .map(|(field, file_name)| {
                format!(
                    "{}{}{}",
                    if let Some(field_name) = &field {
                        field_name
                    } else {
                        ""
                    },
                    if field.is_some() { " -> " } else { "" },
                    file_name
                )
            })
            .collect::<Vec<String>>()
            .join("\n"),
    ))
}

/// Global user_id
static USER_ID: OnceCell<String> = OnceCell::new();

/// Global password
static PASSWORD: OnceCell<String> = OnceCell::new();

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::parse();
    let port = settings.port;
    let base_path = settings.base_path.unwrap_or(PathBuf::new());

    let user_id = random_token(3);
    let password = random_token(21);
    println!("User ID:     {:?}", &user_id);
    println!("Password:    {:?}", &password);
    println!(
        "Curl option: {:?}",
        format!("--user {}:{}", &user_id, &password)
    );
    println!(
        "Header:      {:?}",
        authorization::Basic::new(user_id.clone(), Some(password.clone())).try_into_value()?
    );
    USER_ID
        .set(user_id)
        .map_err(|_| anyhow!("couldn't set global user_id"))?;
    PASSWORD
        .set(password)
        .map_err(|_| anyhow!("couldn't set global password"))?;

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compress::default())
            .wrap(middleware::NormalizePath::trim())
            .wrap(HttpAuthentication::basic(|req, credentials| async move {
                if matches!((
                    USER_ID.get(),
                    PASSWORD.get(),
                    credentials.user_id(),
                    credentials.password()
                ), (
                    Some(user_id),
                    Some(password),
                    given_user_id,
                    Some(given_password)
                ) if user_id == given_user_id && password == given_password)
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
            .app_data(Data::new(base_path.clone()))
            .route("/", post().to(create_file))
            .service(fs::Files::new("/", &base_path))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await?;
    Ok(())
}
