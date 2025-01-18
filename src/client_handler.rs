use crate::message::ChatMessage; // Import ChatMessage from the message module
use dashmap::DashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, RwLock};

// Handles communication with a single client
pub fn handle_client(
    mut stream: TcpStream,
    clients: Arc<DashMap<SocketAddr, TcpStream>>,
    usernames: Arc<DashMap<SocketAddr, String>>,
    chat_history: Arc<RwLock<Vec<ChatMessage>>>,
) -> std::io::Result<()> {
    let peer_addr = stream.peer_addr()?;
    clients.insert(peer_addr, stream.try_clone()?);
    println!("New client: {}", peer_addr);

    let mut buffer = [0; 1024];
    let mut leave_handled = false;

    match stream.read(&mut buffer) {
        Ok(n) if n > 0 => {
            let username = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
            usernames.insert(peer_addr, username.clone());
            println!("Username for {:?} is {}", peer_addr, username);

            {
                let history = chat_history.read().unwrap();
                for old_msg in history.iter() {
                    let serialized = serde_json::to_string(old_msg)?;
                    stream.write_all(format!("{}\n", serialized).as_bytes())?;
                }
            }

            let join_msg = ChatMessage {
                message_type: "join".to_string(),
                username: Some(username.clone()),
                content: format!("{} has entered the chat", username),
            };

            chat_history.write().unwrap().push(join_msg.clone());
            broadcast_message(&clients, &peer_addr, &join_msg);
        }
        _ => {
            println!("Failed to read username from {:?}", peer_addr);
            return Ok(());
        }
    }

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                println!("Client disconnected: {:?}", peer_addr);
                break;
            }
            Ok(n) => {
                let raw_msg = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                if raw_msg == "/list" {
                    let username_list: Vec<String> = usernames
                        .iter()
                        .map(|entry| entry.value().clone())
                        .collect();
                    let list_msg = ChatMessage {
                        message_type: "list".to_string(),
                        username: None,
                        content: format!("Online users: {}", username_list.join(", ")),
                    };
                    let serialized_msg = serde_json::to_string(&list_msg)?;
                    stream.write_all(format!("{}\n", serialized_msg).as_bytes())?;
                    continue;
                }

                let incoming_msg: Result<ChatMessage, _> = serde_json::from_str(&raw_msg);
                match incoming_msg {
                    Ok(parsed_msg) => match parsed_msg.message_type.as_str() {
                        "message" => {
                            let sender_username = usernames
                                .get(&peer_addr)
                                .map(|entry| entry.value().clone())
                                .unwrap_or_else(|| "Unknown".to_string());
                            let chat_msg = ChatMessage {
                                message_type: "message".to_string(),
                                username: Some(sender_username),
                                content: parsed_msg.content,
                            };
                            chat_history.write().unwrap().push(chat_msg.clone());
                            broadcast_message(&clients, &peer_addr, &chat_msg);
                        }
                        "leave" => {
                            broadcast_message(&clients, &peer_addr, &parsed_msg);
                            leave_handled = true;
                        }
                        _ => {}
                    },
                    Err(_) => continue,
                }
            }
            Err(_) => break,
        }
    }

    if !leave_handled {
        let username = usernames
            .remove(&peer_addr)
            .map(|(_, username)| username)
            .unwrap_or_else(|| "Unknown".to_string());
        let leave_msg = ChatMessage {
            message_type: "leave".to_string(),
            username: Some(username.clone()),
            content: format!("{} has left the chat", username),
        };
        broadcast_message(&clients, &peer_addr, &leave_msg);
    }
    clients.remove(&peer_addr);
    println!("Client disconnected: {:?}", peer_addr);
    Ok(())
}

// Broadcast a message to all clients except the sender
fn broadcast_message(
    clients: &Arc<DashMap<SocketAddr, TcpStream>>,
    sender: &SocketAddr,
    message: &ChatMessage,
) {
    let serialized_msg = serde_json::to_string(message).unwrap();
    clients.iter().for_each(|entry| {
        if entry.key() != sender {
            if let Err(e) = entry.value().write_all(serialized_msg.as_bytes()) {
                eprintln!("[ERROR] Failed to send message to {:?}: {}", entry.key(), e);
            }
        }
    });
}
