// Step 1: Define the JSON message structure
// We'll use the `serde` crate for serializing and deserializing JSON.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

// Define the message structure for communication between clients and the server
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChatMessage {
    message_type: String,     // "message", "join", or "leave"
    username: Option<String>, // Username of the sender (None for join/leave notifications)
    content: String,          // The actual message or notification content
}

// Function to handle communication with a connected client
fn handle_client(
    mut stream: TcpStream, // Stream for communicating with the client
    clients: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>, // Shared list of clients
    usernames: Arc<Mutex<HashMap<SocketAddr, String>>>, // Shared map of usernames
    chat_history: Arc<Mutex<Vec<ChatMessage>>>, // Shared vector for message history
) {
    let peer_addr = stream.peer_addr().unwrap(); // Get the client's address
    println!("Started handling client: {:?}", peer_addr);

    let mut buffer = [0; 1024]; // Buffer for reading incoming data

    // Step 1: Read the initial message to get the username
    match stream.read(&mut buffer) {
        Ok(n) if n > 0 => {
            // Extract the username from the buffer
            let username = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
            usernames
                .lock()
                .unwrap()
                .insert(peer_addr, username.clone()); // Save the username
            println!("Username for {:?} is {}", peer_addr, username);

            {
                let history = chat_history.lock().unwrap();
                for old_msg in history.iter() {
                    // Serialize each old message and send it to the new client
                    let serialized = serde_json::to_string(old_msg).unwrap();
                    if let Err(e) = stream.write_all(format!("{}\n", serialized).as_bytes()) {
                        println!("[ERROR] Failed to send history to {:?}: {}", peer_addr, e);
                    }
                }
            }

            // Notify all clients that a new user has joined
            let join_msg = ChatMessage {
                message_type: "join".to_string(),
                username: Some(username.clone()),
                content: format!("{} has entered the chat", username),
            };

            // Also store that "join" event in history if you want.
            chat_history.lock().unwrap().push(join_msg.clone());

            broadcast_message(&clients, &peer_addr, &join_msg);
        }
        _ => {
            println!("Failed to read username from {:?}", peer_addr);
            return;
        }
    }

    // Step 2: Handle messages from the client in a loop
    loop {
        match stream.read(&mut buffer) {
            // stream.read() returns the number of bytes read
            Ok(0) => {
                // Client has disconnected
                println!("Client disconnected: {:?}", peer_addr);
                break;
            }
            Ok(n) => {
                // Successfully read `n` bytes
                let raw_msg = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                println!("[DEBUG] raw_msg = {}", raw_msg);

                // Step 2a: Deserialize the incoming JSON message
                let incoming_msg: Result<ChatMessage, _> = serde_json::from_str(&raw_msg);
                match incoming_msg {
                    Ok(parsed_msg) => {
                        // Extract and handle parsed message
                        let sender_username = usernames
                            .lock()
                            .unwrap()
                            .get(&peer_addr)
                            .map(|s| s.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        println!("[DEBUG] Parsed message: {:?}", parsed_msg);

                        match parsed_msg.message_type.as_str() {
                            "message" => {
                                println!(
                                    "[DEBUG] Sender: {}, Content: {}",
                                    sender_username, parsed_msg.content
                                );
                                let chat_msg = ChatMessage {
                                    message_type: "message".to_string(),
                                    username: Some(sender_username),
                                    content: parsed_msg.content,
                                };

                                // Also store that "message" event in history if you want.
                                chat_history.lock().unwrap().push(chat_msg.clone());

                                broadcast_message(&clients, &peer_addr, &chat_msg);
                            }
                            "join" | "leave" => {
                                println!("[DEBUG] Notification: {}", parsed_msg.content)
                            }
                            _ => println!(
                                "[DEBUG] Unknown message type: {}",
                                parsed_msg.message_type
                            ),
                        }
                    }
                    Err(e) => {
                        println!(
                            "[DEBUG] Failed to parse message from {:?}: {}",
                            peer_addr, e
                        );
                    }
                }
            }
            Err(e) => {
                // Handle the error case
                println!("[ERROR] Failed to read from client {:?}: {}", peer_addr, e);
                break;
            }
        }
    }

    // Step 3: Notify all clients that the user has left
    let leave_msg = ChatMessage {
        message_type: "leave".to_string(),
        username: Some(
            usernames
                .lock()
                .unwrap()
                .get(&peer_addr)
                .unwrap()
                .to_string(),
        ),
        content: format!(
            "{} has left the chat",
            usernames.lock().unwrap().get(&peer_addr).unwrap()
        ),
    };
    broadcast_message(&clients, &peer_addr, &leave_msg);

    // Clean up: Remove the client from the list and usernames
    clients.lock().unwrap().remove(&peer_addr);
    usernames.lock().unwrap().remove(&peer_addr);
    println!("Client disconnected: {:?}", peer_addr);
}

// Function to broadcast a message to all connected clients, except the sender
fn broadcast_message(
    clients: &Arc<Mutex<HashMap<SocketAddr, TcpStream>>>, // Shared list of clients
    sender: &SocketAddr,                                  // Address of the sender
    message: &ChatMessage,                                // The message to broadcast
) {
    let serialized_msg = serde_json::to_string(message).unwrap(); // Serialize the message to JSON
    let mut clients = clients.lock().unwrap();
    for (addr, client) in clients.iter_mut() {
        if addr != sender {
            println!("Serialized message: {}", serialized_msg); // Debug log for the serialized message
            client.write_all(serialized_msg.as_bytes()).unwrap(); // Send the JSON message
            client.write_all(b"\n").unwrap(); // Ensure each message ends with a newline
        }
    }
}

fn main() -> std::io::Result<()> {
    let mut port = 8081; // Starting port for the server
    let listener = loop {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(listener) => {
                println!("Server listening on 127.0.0.1:{}", port);
                break listener;
            }
            Err(_) => {
                println!("Port {} is in use, trying port {}", port, port + 1);
                port += 1; // Try the next port
            }
        }
    };

    // Shared resources: list of clients and their usernames
    let clients = Arc::new(Mutex::new(HashMap::new()));
    let usernames = Arc::new(Mutex::new(HashMap::new()));

    let chat_history = Arc::new(Mutex::new(Vec::new()));

    // Accept incoming client connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer_addr = stream.peer_addr().unwrap(); // Get the client's address
                println!("New client connected: {:?}", peer_addr);

                let clients = Arc::clone(&clients);
                let usernames = Arc::clone(&usernames);
                clients
                    .lock()
                    .unwrap()
                    .insert(peer_addr, stream.try_clone().unwrap());
                let chat_history = Arc::clone(&chat_history);

                // Spawn a new thread to handle the client
                thread::spawn(move || handle_client(stream, clients, usernames, chat_history));
            }
            Err(e) => {
                println!("Failed to accept connection: {}", e);
            }
        }
    }

    Ok(())
}
