mod gpt_client;
mod client_handler;
mod client;

use tokio::net::UnixListener;
use std::fs;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use clap::Parser;
use tokio::task::JoinHandle;
use tokio::time;
use crate::gpt_client::GptEngine;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {

    #[arg(short, long, env, value_name = "SOCKET_DIR")]
    socket_dir: PathBuf,

    #[arg(short, long, env, value_name = "TOKEN")]
    token: String,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let (mut socket_dir_string, token) = get_configuration()?;

    let socket_paths = [
        ("gpt4_32k.sock", GptEngine::Gpt4_32k(token.clone())),
        ("gpt4.sock", GptEngine::Gpt4(token.clone())),
        ("gpt3_5_turbo.sock", GptEngine::Gpt35Turbo(token)),
    ];

    let should_terminate = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();
    for (socket_path, engine) in &socket_paths {
        let socket_file = socket_dir_string.clone() + socket_path;
        let socket = setup_server_socket(&socket_file)?;
        let terminate_signal = should_terminate.clone();
        let join_handle = start_server_thread(engine.to_owned(), socket, terminate_signal);
        println!("Server listening on {} for thread {:?}", socket_file, join_handle);
        handles.push(join_handle);
    }

    wait_for_signint_or_sigterm().await?;

    // Signal all threads to terminate and join them.
    should_terminate.store(true, Ordering::Release);
    for handle in handles {
        println!("Waiting for the {:?} thread to join...", handle);
        if let Err(e) = handle.await {
            eprintln!("An error occurred while joining a thread: {}", e);
        }
    }

    // Delete all sockets
    for (socket_path, _) in socket_paths {
        let socket_file = socket_dir_string.clone() + socket_path;
        println!("Deleting socket {}", socket_file);
        if let Err(e) = fs::remove_file(&socket_file) {
            eprintln!("An error occurred while deleting socket {}: {}", socket_file, e);
        }
    }

    println!("\nExiting application...");

    Ok(())
}

async fn wait_for_signint_or_sigterm() -> Result<(), String> {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).map_err(|e| e.to_string())?;

    tokio::select! {
        _ = ctrl_c => println!("SIGINT received"),
        _ = sigterm.recv() => println!("SIGTERM received"),
    }
    Ok(())
}

fn setup_server_socket(socket_file: &String) -> Result<(UnixListener), String> {
    fs::remove_file(&socket_file).ok();
    let listener = UnixListener::bind(&socket_file).map_err(|e| e.to_string())?;

    // Give the socket a mode of 660 to allow users in same group to access.
    let permissions = Permissions::from_mode(0o660);
    fs::set_permissions(socket_file, permissions).map_err(|e| e.to_string())?;

    Ok(listener)
}

fn start_server_thread(engine: GptEngine, listener: UnixListener, should_shutdown: Arc<AtomicBool>) -> JoinHandle<()> {
    let handle = tokio::spawn(async move {

        let mut client_handles = Vec::new();

        loop {
            // Check for shutdown signal
            if should_shutdown.load(Ordering::Acquire) {
                break;
            }

            remove_completed_handles(&mut client_handles);

            let accept_future = listener.accept();
            let result = time::timeout(Duration::from_secs(2), accept_future).await;

            let (stream, _) = match result {
                Ok(Ok(connection)) => connection,
                Ok(Err(e)) => {
                    eprintln!("Failed to accept connection: {}", e);
                    continue;
                },
                Err(_) => {
                    // Timeout occurred, continue as normal
                    continue;
                }
            };

            let engine_clone_inner = engine.clone();
            client_handles.push(tokio::spawn(async move {
                if let Err(e) = client_handler::handle_client(stream, engine_clone_inner).await {
                    eprintln!("An error occurred while handling the client: {}", e);
                }
            }));
        }
    });
    return handle;
}

fn remove_completed_handles(client_handles: &mut Vec<JoinHandle<()>>) {
    let mut active_handles = Vec::new();

    for handle in client_handles.drain(..) {
        if handle.is_finished() {
            active_handles.push(handle);
        }
    }

    client_handles.extend(active_handles);
}

fn get_configuration() -> Result<(String, String), String> {
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

    if !socket_dir_string.ends_with("/") {
        socket_dir_string = socket_dir_string + "/";
    }

    let token = cli.token;
    Ok((socket_dir_string, token))
}
