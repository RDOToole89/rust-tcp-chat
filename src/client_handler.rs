// client_handler.rs
use crate::errors::{ChatResult, ChatServerError};
use crate::message::{ChatMessage, ChatMessageType, CommandType};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, RwLock};

/// Handles communication with a single client.
pub fn handle_client(
    mut stream: TcpStream,
    clients: Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    usernames: Arc<RwLock<HashMap<SocketAddr, String>>>,
    chat_history: Arc<RwLock<Vec<ChatMessage>>>,
) -> ChatResult<()> {
    let peer_addr = stream.peer_addr()?;
    println!("Handling client: {:?}", peer_addr);

    register_client(&stream, &clients, &usernames, peer_addr)?;

    let username = get_client_username(&mut stream, peer_addr)?;
    println!("Client registered as '{}'", username);

    send_chat_history(&mut stream, &chat_history)?;
    broadcast_join_message(&clients, &usernames, peer_addr, &username, &chat_history)?;

    handle_client_messages(
        &mut stream,
        &clients,
        &usernames,
        &chat_history,
        peer_addr,
        &username,
    )?;

    cleanup_client(&clients, &usernames, peer_addr);
    Ok(())
}

/// Registers the client in the shared `clients` map.
fn register_client(
    stream: &TcpStream,
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    _usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,
    peer_addr: SocketAddr,
) -> ChatResult<()> {
    let mut clients_lock = clients.write()?;
    clients_lock.insert(peer_addr, stream.try_clone()?);
    Ok(())
}

/// Reads and returns the username from the client.
fn get_client_username(stream: &mut TcpStream, peer_addr: SocketAddr) -> ChatResult<String> {
    let mut buffer = [0; 1024];
    let raw_message = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => String::from_utf8_lossy(&buffer[..n]).trim().to_string(),
        _ => return Err(ChatServerError::ClientDisconnected(peer_addr.to_string())),
    };

    // Deserialize the JSON message
    let chat_message: ChatMessage = serde_json::from_str(&raw_message)
        .map_err(|_| ChatServerError::ClientDisconnected(peer_addr.to_string()))?;

    // Extract the username from the message
    chat_message
        .username
        .ok_or_else(|| ChatServerError::ClientDisconnected(peer_addr.to_string()))
}

/// Sends the chat history to the client.
fn send_chat_history(
    stream: &mut TcpStream,
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,
) -> ChatResult<()> {
    let history = chat_history.read()?;
    for msg in history.iter() {
        send_message_to_client(stream, msg)?;
    }
    Ok(())
}

/// Broadcasts a "join" message to all clients.
fn broadcast_join_message(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,
    peer_addr: SocketAddr,
    username: &str,
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,
) -> ChatResult<()> {
    {
        let mut usernames_lock = usernames.write()?;
        usernames_lock.insert(peer_addr, username.to_string());
    }

    broadcast_system_message(
        clients,
        chat_history,
        peer_addr,
        username,
        ChatMessageType::Join,
        format!("{} has joined the chat", username),
    )?;
    Ok(())
}

fn broadcast_system_message(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,
    sender: SocketAddr,
    username: &str,
    message_type: ChatMessageType,
    content: String,
) -> ChatResult<ChatMessage> {
    let msg = ChatMessage {
        message_type,
        username: Some(username.to_string()),
        content,
    };
    broadcast_message(clients, sender, &msg, chat_history);
    Ok(msg)
}

/// Handles incoming messages from the client.
fn handle_client_messages(
    stream: &mut TcpStream,
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,
    peer_addr: SocketAddr,
    username: &str,
) -> ChatResult<()> {
    let mut buffer = [0; 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Connection closed
            Ok(n) => {
                let raw_msg = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                if let Ok(chat_msg) = serde_json::from_str::<ChatMessage>(&raw_msg) {
                    handle_parsed_message(
                        stream,
                        clients,
                        usernames,
                        chat_history,
                        peer_addr,
                        username,
                        chat_msg,
                    )?;
                } else {
                    eprintln!("Failed to parse message: {}", raw_msg);
                }
            }
            Err(_) => break,
        }
    }
    Ok(())
}

/// Handles a parsed `ChatMessage` from the client.
fn handle_parsed_message(
    stream: &mut TcpStream,
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,
    peer_addr: SocketAddr,
    username: &str,
    chat_msg: ChatMessage,
) -> ChatResult<()> {
    match chat_msg.message_type {
        ChatMessageType::Message => {
            let msg = ChatMessage {
                message_type: ChatMessageType::Message,
                username: Some(username.to_string()),
                content: chat_msg.content,
            };
            broadcast_message(clients, peer_addr, &msg, chat_history);
        }
        ChatMessageType::Command(CommandType::List) => {
            send_user_list(stream, usernames)?;
        }
        ChatMessageType::Command(CommandType::Quit) | ChatMessageType::Leave => {
            handle_client_disconnect(
                stream,
                clients,
                usernames,
                peer_addr,
                username,
                &chat_msg.message_type,
                chat_history,
            )?;
        }
        _ => {
            eprintln!("Unhandled message type: {:?}", chat_msg.message_type);
        }
    }
    Ok(())
}

/// Sends the list of online users to the client.
fn send_user_list(
    stream: &mut TcpStream,
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,
) -> ChatResult<()> {
    let users = {
        // Acquire a lock on the usernames hashmap and collect all usernames
        let usernames_lock = usernames.read()?;
        if usernames_lock.is_empty() {
            eprintln!("DEBUG: No users found in usernames map."); // Debugging log
        } else {
            println!(
                "DEBUG: Found users in usernames map: {:?}",
                usernames_lock.values().cloned().collect::<Vec<_>>()
            );
        }
        usernames_lock.values().cloned().collect::<Vec<_>>()
    };

    // Create a response message with the list of online users
    let list_msg = ChatMessage {
        message_type: ChatMessageType::Command(CommandType::List),
        username: None,
        content: if users.is_empty() {
            "No users online.".to_string()
        } else {
            format!("Online users: {}", users.join(", "))
        },
    };

    // Send the list message back to the requesting client
    send_message_to_client(stream, &list_msg)
}

/// Handles client disconnects by broadcasting a "leave" message and cleaning up.
fn handle_client_disconnect(
    stream: &mut TcpStream,
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,
    peer_addr: SocketAddr,
    username: &str,
    message_type: &ChatMessageType,
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,
) -> ChatResult<()> {
    let leave_msg = broadcast_system_message(
        clients,
        chat_history,
        peer_addr,
        username,
        message_type.clone(),
        format!("{} has left the chat", username),
    )?;
    send_message_to_client(stream, &leave_msg)?;

    // Remove the client from shared state
    {
        let mut clients_lock = clients.write()?;
        clients_lock.remove(&peer_addr);

        let mut usernames_lock = usernames.write()?;
        usernames_lock.remove(&peer_addr);
    }

    Ok(())
}

/// Removes a client from the shared state after disconnection.
fn cleanup_client(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,
    peer_addr: SocketAddr,
) {
    clients.write().ok().map(|mut lock| lock.remove(&peer_addr));
    usernames
        .write()
        .ok()
        .map(|mut lock| lock.remove(&peer_addr));
}

/// Sends a message to a single client.
fn send_message_to_client(stream: &mut TcpStream, message: &ChatMessage) -> ChatResult<()> {
    let serialized_msg = serde_json::to_string(message)?;
    stream.write_all(format!("{}\n", serialized_msg).as_bytes())?;
    Ok(())
}
/// Broadcasts a message to all clients except the sender and updates the chat history.
fn broadcast_message(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>,
    sender: SocketAddr,
    message: &ChatMessage,
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,
) {
    // Add the message to the chat history
    {
        let mut history_lock = chat_history.write().unwrap();
        history_lock.push(message.clone());
    }

    let serialized = serde_json::to_string(message).unwrap_or_default();
    let mut failed_clients = vec![];

    // Use a read lock to access the clients
    {
        let clients_lock = clients.read().unwrap();
        for (&addr, client) in clients_lock.iter() {
            if addr != sender {
                if let Ok(mut writable_client) = client.try_clone() {
                    if let Err(_) =
                        writable_client.write_all(format!("{}\n", serialized).as_bytes())
                    {
                        failed_clients.push(addr); // Collect failed clients
                    }
                } else {
                    failed_clients.push(addr);
                }
            }
        }
    }

    // Clean up failed clients
    if !failed_clients.is_empty() {
        let mut clients_lock = clients.write().unwrap();
        for addr in failed_clients {
            eprintln!("Removing failed client: {}", addr);
            clients_lock.remove(&addr);
        }
    }
}
