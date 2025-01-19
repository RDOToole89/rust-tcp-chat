// server.rs
mod client_handler;
mod errors;
mod message;

use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use client_handler::handle_client;
use errors::ChatResult;

fn main() -> ChatResult<()> {
    let listener = TcpListener::bind("127.0.0.1:8081")
        .map_err(|_| errors::ChatServerError::NoAvailablePorts)?;
    println!("Server is running on 127.0.0.1:8081");

    let clients = Arc::new(Mutex::new(HashMap::new()));
    let usernames = Arc::new(Mutex::new(HashMap::new()));
    let chat_history = Arc::new(Mutex::new(Vec::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let clients = Arc::clone(&clients);
                let usernames = Arc::clone(&usernames);
                let chat_history = Arc::clone(&chat_history);

                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, clients, usernames, chat_history) {
                        eprintln!("Error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Failed to accept connection: {}", e),
        }
    }
    Ok(())
}
