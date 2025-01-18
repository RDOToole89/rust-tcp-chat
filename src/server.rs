mod client_handler;
mod errors;
mod message;

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use client_handler::handle_client;
use errors::ChatResult; // <--- Only ChatResult used here
use message::ChatMessage; // <--- Our shared ChatMessage struct

fn main() -> ChatResult<()> {
    let mut port = 8081;
    // Try binding to 127.0.0.1:port, increment if in use
    let listener = loop {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(lst) => {
                println!("Server listening on 127.0.0.1:{}", port);
                break lst;
            }
            Err(_) => {
                println!("Port {} in use. Trying {}", port, port + 1);
                port += 1;
            }
        }
    };

    // Shared data
    let clients: Arc<Mutex<HashMap<SocketAddr, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    let usernames: Arc<Mutex<HashMap<SocketAddr, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let chat_history: Arc<Mutex<Vec<ChatMessage>>> = Arc::new(Mutex::new(Vec::new()));

    // Accept loop
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer_addr = match stream.peer_addr() {
                    Ok(addr) => addr,
                    Err(e) => {
                        eprintln!("Could not get peer address: {}", e);
                        continue;
                    }
                };
                println!("New client connected: {:?}", peer_addr);

                // Insert the new client into the map right away
                {
                    let mut lock = clients.lock().unwrap();
                    lock.insert(peer_addr, stream.try_clone()?);
                }

                // Clone Arcs for the new thread
                let clients_clone = Arc::clone(&clients);
                let usernames_clone = Arc::clone(&usernames);
                let history_clone = Arc::clone(&chat_history);

                // Spawn a client handler
                thread::spawn(move || {
                    if let Err(e) =
                        handle_client(stream, clients_clone, usernames_clone, history_clone)
                    {
                        eprintln!("Error handling client {:?}: {:?}", peer_addr, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
    Ok(())
}
