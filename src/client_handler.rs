// src/client_handler.rs
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};

use crate::errors::{ChatResult, ChatServerError}; // <--- Use custom errors
use crate::message::{ChatMessage, ChatMessageType};

/// This function is spawned in a thread for each connected client.
///
/// - `clients` holds all connected sockets (by their address).
/// - `usernames` tracks each client’s chosen username.
/// - `chat_history` is a list of past messages for new clients to see.
pub fn handle_client(
    mut stream: TcpStream,
    clients: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,
    usernames: Arc<Mutex<HashMap<SocketAddr, String>>>,
    chat_history: Arc<Mutex<Vec<ChatMessage>>>,
) -> ChatResult<()> {
    let peer_addr = stream.peer_addr()?;
    println!("Started handling client: {:?}", peer_addr);

    // Insert the stream (also done in `server.rs` right after accept, but okay to do here)
    {
        let mut lock = clients.lock().unwrap();
        lock.insert(peer_addr, stream.try_clone()?);
    }

    let mut buffer = [0; 1024];
    let mut leave_handled = false;

    // 1. Read the username line
    match stream.read(&mut buffer) {
        Ok(n) if n > 0 => {
            let username = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
            {
                let mut lock = usernames.lock().unwrap();
                lock.insert(peer_addr, username.clone());
            }
            println!("Username for {:?} is {}", peer_addr, username);

            // 2. Send existing chat history
            {
                let history = chat_history.lock().unwrap();
                for old_msg in history.iter() {
                    let serialized = serde_json::to_string(old_msg)?;
                    stream.write_all(serialized.as_bytes())?;
                    stream.write_all(b"\n")?;
                }
            }

            // 3. Broadcast a "join" message
            let join_msg = ChatMessage {
                message_type: ChatMessageType::Join,
                username: Some(username.clone()),
                content: format!("{} has entered the chat", username),
            };
            {
                let mut h = chat_history.lock().unwrap();
                h.push(join_msg.clone());
            }
            broadcast_message(&clients, &peer_addr, &join_msg);
        }
        _ => {
            println!("Failed to read username from {:?}", peer_addr);
            return Ok(()); // Gracefully return if no username
        }
    }

    // 4. Main read loop
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                return Err(ChatServerError::ClientDisconnected(
                    "Client closed connection".to_string(),
                ))
            }
            Ok(n) => {
                let raw_msg = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                if raw_msg.is_empty() {
                    continue;
                }

                // Special command: /list
                if raw_msg == "/list" {
                    let username_list = {
                        let lock = usernames.lock().unwrap();
                        lock.values().cloned().collect::<Vec<String>>()
                    };
                    let list_msg = ChatMessage {
                        message_type: ChatMessageType::List,
                        username: None,
                        content: format!("Online users: {}", username_list.join(", ")),
                    };
                    let serialized = serde_json::to_string(&list_msg)?;
                    stream.write_all(serialized.as_bytes())?;
                    stream.write_all(b"\n")?;
                    continue;
                }

                // Otherwise, parse JSON
                let parsed_msg: Result<ChatMessage, _> = serde_json::from_str(&raw_msg);
                match parsed_msg {
                    Ok(msg) => {
                        let sender_username = {
                            let lock = usernames.lock().unwrap();
                            lock.get(&peer_addr)
                                .cloned()
                                .unwrap_or_else(|| "Unknown".to_string())
                        };

                        match msg.message_type {
                            ChatMessageType::Message => {
                                // Normal chat
                                let chat_msg = ChatMessage {
                                    message_type: ChatMessageType::Message,
                                    username: Some(sender_username),
                                    content: msg.content,
                                };
                                {
                                    let mut lock = chat_history.lock().unwrap();
                                    lock.push(chat_msg.clone());
                                }
                                broadcast_message(&clients, &peer_addr, &chat_msg);
                            }
                            ChatMessageType::Leave => {
                                // Mark that we handled "leave"
                                broadcast_message(&clients, &peer_addr, &msg);
                                leave_handled = true;
                                break;
                            }
                            ChatMessageType::Join => {
                                broadcast_message(&clients, &peer_addr, &msg);
                            }
                            _ => {
                                println!("[DEBUG] Unknown message type: {:?}", msg.message_type);
                            }
                        }
                    }
                    Err(e) => {
                        println!("[DEBUG] Failed to parse message: {} => {}", raw_msg, e);
                    }
                }
            }
            Err(e) => {
                println!("[ERROR] Failed to read from {:?}: {}", peer_addr, e);
                break;
            }
        }
    }

    // If not handled by user sending a "leave", do it now
    if !leave_handled {
        let username = {
            let mut lock = usernames.lock().unwrap();
            lock.remove(&peer_addr)
                .unwrap_or_else(|| "Unknown".to_string())
        };
        let leave_msg = ChatMessage {
            message_type: ChatMessageType::Leave,
            username: Some(username.clone()),
            content: format!("{} has left the chat", username),
        };
        broadcast_message(&clients, &peer_addr, &leave_msg);
    }

    // Final cleanup
    {
        let mut lock = clients.lock().unwrap();
        lock.remove(&peer_addr);
    }

    println!("Finished handling client: {:?}", peer_addr);
    Ok(())
}

/// Broadcast `message` to all clients except `sender`.
pub fn broadcast_message(
    clients: &Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,
    sender: &SocketAddr,
    message: &ChatMessage,
) {
    let serialized = match serde_json::to_string(message) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to serialize message: {}", e);
            return;
        }
    };
    let mut lock = clients.lock().unwrap();
    for (addr, client) in lock.iter_mut() {
        if addr != sender {
            // Write JSON + newline
            if let Err(e) = client.write_all(serialized.as_bytes()) {
                eprintln!("[ERROR] broadcasting to {:?}: {}", addr, e);
            }
            if let Err(e) = client.write_all(b"\n") {
                eprintln!("[ERROR] broadcasting newline to {:?}: {}", addr, e);
            }
        }
    }
}
