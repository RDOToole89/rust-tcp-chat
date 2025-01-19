// Module imports for client handling, error handling, and message types.
mod client_handler;
mod errors;
mod message;

use client_handler::handle_client; // Function to handle each client connection.
use ctrlc::set_handler; // For handling Ctrl+C to gracefully shut down the server.
use errors::ChatResult; // Custom result type for error handling.
use message::ChatMessage; // Message type for communication.
use std::collections::HashMap; // Used to store client connections and usernames.
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream}; // Networking utilities.
use std::sync::atomic::{AtomicBool, Ordering}; // Atomic flag for thread-safe shutdown.
use std::sync::{Arc, RwLock}; // Shared data structures for thread-safe access.
use std::thread; // For spawning threads for each client.

fn main() -> ChatResult<()> {
    // Initialize the logger with Info-level logging for debugging and operational clarity.
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info) // Set the global log level to Info.
        .init();

    // Bind the server to a local address and port (127.0.0.1:8081).
    // Wrap `TcpListener` in an `Arc` so it can be shared across threads.
    let listener = Arc::new(
        TcpListener::bind("127.0.0.1:8081")
            .map_err(|_| errors::ChatServerError::NoAvailablePorts)?, // Handle binding errors.
    );
    log::info!("Server is running on 127.0.0.1:8081");

    // Shared structures for managing clients, usernames, and chat history.
    let clients = Arc::new(RwLock::new(HashMap::<SocketAddr, TcpStream>::new())); // Client connections.
    let usernames = Arc::new(RwLock::new(HashMap::<SocketAddr, String>::new())); // Usernames by address.
    let chat_history = Arc::new(RwLock::new(Vec::<ChatMessage>::new())); // Chat message history.

    // Atomic flag for server shutdown, allowing threads to check if the server is shutting down.
    let is_shutting_down = Arc::new(AtomicBool::new(false));

    // Handle Ctrl+C signal to gracefully shut down the server.
    let clients_clone = Arc::clone(&clients); // Clone `clients` to use in the signal handler.
    let is_shutting_down_clone = Arc::clone(&is_shutting_down); // Clone the shutdown flag.
    set_handler(move || {
        if is_shutting_down_clone.load(Ordering::SeqCst) {
            return; // Prevent multiple shutdown triggers.
        }
        is_shutting_down_clone.store(true, Ordering::SeqCst); // Set the shutdown flag.
        log::info!("Shutting down server...");

        let clients_lock = clients_clone.read().unwrap(); // Read lock to safely access `clients`.
        for (_, client) in clients_lock.iter() {
            // Gracefully close each client connection.
            if let Err(e) = client.shutdown(Shutdown::Both) {
                log::error!("Failed to shutdown client connection: {}", e);
            }
        }
        std::process::exit(0); // Terminate the process.
    })
    .expect("Error setting Ctrl+C handler");

    // Main loop for accepting client connections.
    for stream in listener.incoming() {
        // If shutdown is triggered, exit the loop.
        if is_shutting_down.load(Ordering::SeqCst) {
            break;
        }

        // Match the incoming connection result.
        match stream {
            Ok(stream) => {
                // Clone shared structures for each new thread.
                // Clone created a new reference to the same data, not a new copy.
                let clients = Arc::clone(&clients);
                let usernames = Arc::clone(&usernames);
                let chat_history = Arc::clone(&chat_history);

                // Spawn a new thread to handle the client.
                thread::spawn(move || {
                    // Handle the client connection and log any errors.
                    if let Err(e) = handle_client(stream, clients, usernames, chat_history) {
                        log::error!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                // Log errors while accepting connections.
                log::error!("Failed to accept connection: {}", e);
                if is_shutting_down.load(Ordering::SeqCst) {
                    break; // Exit loop if shutdown is triggered.
                }
            }
        }
    }

    // Log server shutdown after exiting the loop.
    log::info!("Server has shut down.");
    Ok(())
}
