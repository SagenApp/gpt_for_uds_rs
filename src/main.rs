mod gpt_client;
mod client_handler;
mod client;

use tokio::net::UnixListener;
use std::fs;
use std::path::PathBuf;
use clap::Parser;
use crate::gpt_client::GptEngine;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {

    #[arg(short, long, env, value_name = "SOCKET_DIR", default_value = "/tmp/")]
    socket_dir: PathBuf,

    #[arg(short, long, env, value_name = "TOKEN")]
    token: String,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();

    if !cli.socket_dir.is_absolute() {
        return Err("Socket directory must be an absolute path!".to_string());
    }
    if !cli.socket_dir.is_dir() {
        return Err("Socket directory must be a directory!".to_string());
    }
    if !cli.socket_dir.exists() {
        return Err("Socket directory must exist!".to_string());
    }

    let mut socket_dir_string: String = match cli.socket_dir.to_str() {
        Some(s) => s.to_string(),
        None => return Err("Socket directory must be a valid UTF-8 string!".to_string()),
    };

    if (!socket_dir_string.ends_with("/")) {
        socket_dir_string = socket_dir_string + "/";
    }

    let token = cli.token;
    let socket_paths = [
        ("rust_uds_gpt4_32k.sock", GptEngine::Gpt4_32k(token.clone())),
        ("rust_uds_gpt4.sock", GptEngine::Gpt4(token.clone())),
        ("rust_uds_gpt3_5_turbo.sock", GptEngine::Gpt35Turbo(token)),
    ];

    for (socket_path, engine) in socket_paths {
        fs::remove_file(socket_path).ok();
        let listener = UnixListener::bind(socket_dir_string.clone() + socket_path).map_err(|e| e.to_string())?;

        println!("Server listening on {}", socket_path);

        let engine_clone = engine.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(connection) => connection,
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                        continue;
                    }
                };

                let engine_clone_inner = engine_clone.clone();
                tokio::spawn(async move {
                    if let Err(e) = client_handler::handle_client(stream, engine_clone_inner).await {
                        eprintln!("An error occurred while handling the client: {}", e);
                    }
                });
            }
        });
    }

    let ctrl_c = tokio::signal::ctrl_c();
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).map_err(|e| e.to_string())?;

    tokio::select! {
        _ = ctrl_c => println!("SIGINT received"),
        _ = sigterm.recv() => println!("SIGTERM received"),
    }

    println!("Exiting application...");

    Ok(())
}
