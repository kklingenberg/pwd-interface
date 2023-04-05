use anyhow::{Context, Error, Result};
use core::time::Duration;
use pwd_interface::{bundler, token};
use reqwest::blocking::multipart::Form;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use std::io::{stdin, stdout, Write};
use std::path::Path;
use tempfile::NamedTempFile;

fn welcome() {
    println!(
        r#"PWD-interface Client

Type commands to configure the connection to the server and transfer
the current folder to or from the PWD instance.

Type 'help' for a list of commands."#
    );
}

fn prompt() -> Result<()> {
    print!("\npwd-client> ");
    stdout().flush()?;
    Ok(())
}

fn help() {
    println!(
        r#"Commands:

help          Show this message
server        Show the server URL
server <url>  Set the server URL to <url>
proxy         Show whether the system proxy is being used
proxy <bool>  Set whether the system proxy is used
token         Show the token
token <key>   Set the token to <key>
push          Push the current local directory to the server
pull          Pull the server's directory into the current local directory
exit          Close this pwd-client session"#
    );
}

/// Execute a push to the server
fn push(
    client: &Client,
    bundler: &mut bundler::Bundler,
    working_directory: &Path,
    server: &str,
    token: &str,
) {
    println!("Bundling current directory...");
    match bundler.make(working_directory) {
        Ok(bundle) => {
            println!("Pushing bundle to server...");
            match Form::new()
                .file("file", bundle)
                .map_err(Error::new)
                .and_then(|form| {
                    token::salt_timed(token)
                        .context("Couldn't produce an authentication token")
                        .and_then(|auth_token| {
                            client
                                .post(server)
                                .basic_auth(auth_token, None::<&str>)
                                .multipart(form)
                                .send()
                                .map_err(Error::new)
                        })
                }) {
                Ok(response) if response.status() == StatusCode::CREATED => {
                    println!("Push accepted!");
                }
                Ok(response) => {
                    println!(
                        "There were issues with the push request: status {:?}",
                        response.status()
                    );
                }
                Err(e) => {
                    println!("There were issues with the push request: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("Couldn't make a bundle of the current directory: {:?}", e);
        }
    }
}

/// Execute a pull from the server
fn pull(
    client: &Client,
    bundler: &mut bundler::Bundler,
    working_directory: &Path,
    server: &str,
    token: &str,
) {
    println!("Pulling bundle from server...");
    match token::salt_timed(token)
        .context("Couldn't produce an authentication token")
        .and_then(|auth_token| {
            client
                .get(server)
                .basic_auth(auth_token, None::<&str>)
                .send()
                .map_err(Error::new)
        }) {
        Ok(mut response) if response.status() == StatusCode::OK => {
            match NamedTempFile::new()
                .context("Couldn't create temporary file to hold the pulled bundle")
                .and_then(|mut file| {
                    response
                        .copy_to(&mut file)
                        .context("Couldn't persist the pulled bundle")
                        .and_then(|_| {
                            println!("Extracting bundle...");
                            bundler.extract(file.path(), working_directory)
                        })
                }) {
                Ok(_) => {
                    println!("Pull done!");
                }
                Err(e) => {
                    println!("Couldn't extract the pulled bundle: {:?}", e);
                }
            }
        }
        Ok(response) => {
            println!(
                "There were issues with the pull request: status {:?}",
                response.status()
            );
        }
        Err(e) => {
            println!("There were issues with the pull request: {:?}", e);
        }
    }
}

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// REPL
fn main() -> Result<()> {
    let mut server = String::from("http://localhost");
    let mut proxy = false;
    let mut token = String::from("not-set");
    let mut input = String::new();
    let working_directory = Path::new(".");
    let proxied_client = Client::builder()
        .user_agent(APP_USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()?;
    let direct_client = Client::builder()
        .user_agent(APP_USER_AGENT)
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .build()?;
    let mut bundler = bundler::Bundler::new();

    welcome();
    prompt()?;
    loop {
        stdin().read_line(&mut input)?;
        let clean = input.trim();
        if !input.ends_with('\n') {
            println!();
        }
        if clean == "exit" || clean.is_empty() {
            println!("Bye!");
            break;
        } else if clean == "help" {
            help();
            prompt()?;
        } else if clean == "server" {
            println!("The value of the server URL is: {:?}", server);
            prompt()?;
        } else if clean.starts_with("server ") {
            server.clear();
            server.push_str(clean.strip_prefix("server ").unwrap().trim());
            println!("Set the value of the server URL to: {:?}", server);
            prompt()?;
        } else if clean == "proxy" {
            println!("The value of the proxy flag is: {:?}", proxy);
            prompt()?;
        } else if clean.starts_with("proxy ") {
            let proxy_value = clean.strip_prefix("proxy ").unwrap().trim();
            if proxy_value == "true" {
                proxy = true;
                println!("Set the value of the proxy flag to: {:?}", proxy);
            } else if proxy_value == "false" {
                proxy = false;
                println!("Set the value of the proxy flag to: {:?}", proxy);
            } else {
                println!("Invalid value given to the proxy flag. Use 'true' or 'false' only.");
            }
            prompt()?;
        } else if clean == "token" {
            println!("The value of the token is: {:?}", token);
            prompt()?;
        } else if clean.starts_with("token ") {
            token.clear();
            token.push_str(clean.strip_prefix("token ").unwrap().trim());
            println!("Set the value of the token to: {:?}", token);
            prompt()?;
        } else if clean == "push" {
            push(
                if proxy {
                    &proxied_client
                } else {
                    &direct_client
                },
                &mut bundler,
                working_directory,
                &server,
                &token,
            );
            prompt()?;
        } else if clean == "pull" {
            pull(
                if proxy {
                    &proxied_client
                } else {
                    &direct_client
                },
                &mut bundler,
                working_directory,
                &server,
                &token,
            );
            prompt()?;
        } else {
            println!("Invalid command. Use 'help' for a list of valid options.");
            prompt()?;
        }
        input.clear();
    }
    Ok(())
}
