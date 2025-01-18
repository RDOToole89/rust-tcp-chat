mod client_handler;
mod message; // Import the message module // Import the client handler module

use client_handler::handle_client; // Import the handle_client function
use dashmap::DashMap;
use std::net::TcpListener;
use std::sync::{Arc, RwLock};
use std::thread; // Import the ChatMessage struct

fn main() -> std::io::Result<()> {
    let mut port = 8081;
    let listener = loop {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(listener) => {
                println!("Server listening on 127.0.0.1:{}", port);
                break listener;
            }
            Err(_) => {
                println!("Port {} is in use, trying port {}", port, port + 1);
                port += 1;
            }
        }
    };

    // Shared state for the server
    let clients = Arc::new(DashMap::new()); // Thread-safe map of client addresses to streams
    let usernames = Arc::new(DashMap::new()); // Thread-safe map of client addresses to usernames
    let chat_history = Arc::new(RwLock::new(Vec::new())); // Shared chat history with RwLock

    // Accept incoming client connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let clients = Arc::clone(&clients);
                let usernames = Arc::clone(&usernames);
                let chat_history = Arc::clone(&chat_history);
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, clients, usernames, chat_history) {
                        eprintln!("Error in client thread: {:?}", e);
                    }
                });
            }
            Err(e) => println!("Failed to accept connection: {}", e),
        }
    }

    Ok(())
}
