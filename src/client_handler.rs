// client_handler.rs
use crate::errors::{ChatResult, ChatServerError}; // Custom result and error types for handling errors.
use crate::message::{ChatMessage, ChatMessageType, CommandType}; // Chat message structure and related enums.
use std::collections::HashMap; // Used for storing client connections and usernames.
use std::io::{Read, Write}; // For reading and writing to TCP streams.
use std::net::{SocketAddr, TcpStream}; // Networking primitives for managing client connections.
use std::sync::{Arc, RwLock}; // Thread-safe shared state using reference counting and read-write locks.

/// Handles communication with a single client.
/// Handles communication with a single client.
pub fn handle_client(
    mut stream: TcpStream, // TCP stream for communication with the client.
    clients: Arc<RwLock<HashMap<SocketAddr, TcpStream>>>, // Shared map of connected clients.
    usernames: Arc<RwLock<HashMap<SocketAddr, String>>>, // Shared map of client usernames.
    chat_history: Arc<RwLock<Vec<ChatMessage>>>, // Shared chat history.
) -> ChatResult<()> {
    let peer_addr = stream.peer_addr()?; // Get the client's address for identification.
    println!("Handling client: {:?}", peer_addr);

    // Add the client to the shared clients map.
    register_client(&stream, &clients, &usernames, peer_addr)?;

    // Retrieve and validate the username from the client.
    let username = get_client_username(&mut stream, peer_addr)?;
    println!("Client registered as '{}'", username);

    // Send the chat history to the client after they connect.
    send_chat_history(&mut stream, &chat_history)?;

    // Notify all other clients that a new client has joined the chat.
    broadcast_join_message(&clients, &usernames, peer_addr, &username, &chat_history)?;

    // Start listening for messages from the client.
    handle_client_messages(
        &mut stream,
        &clients,
        &usernames,
        &chat_history,
        peer_addr,
        &username,
    )?;

    // Clean up the client after they disconnect.
    cleanup_client(&clients, &usernames, peer_addr);
    Ok(())
}

/// Registers the client in the shared `clients` map.
fn register_client(
    stream: &TcpStream,                                    // The client's TCP stream.
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>, // Shared map of connected clients.
    _usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>, // (Not used here) Shared map of usernames.
    peer_addr: SocketAddr,                                 // The client's address.
) -> ChatResult<()> {
    let mut clients_lock = clients.write()?; // Acquire a write lock to modify the clients map.
    clients_lock.insert(peer_addr, stream.try_clone()?); // Add the client with a cloned TCP stream.
    Ok(())
}

/// Reads and returns the username from the client.
fn get_client_username(stream: &mut TcpStream, peer_addr: SocketAddr) -> ChatResult<String> {
    let mut buffer = [0; 1024]; // Buffer to store the received data.
    let raw_message = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => String::from_utf8_lossy(&buffer[..n]).trim().to_string(), // Convert to string.
        _ => return Err(ChatServerError::ClientDisconnected(peer_addr.to_string())), // Handle client disconnection.
    };

    // Parse the JSON message and extract the username.
    let chat_message: ChatMessage = serde_json::from_str(&raw_message)
        .map_err(|_| ChatServerError::InvalidMessage(peer_addr.to_string()))?;
    chat_message
        .username
        .ok_or_else(|| ChatServerError::MissingUsername(peer_addr.to_string())) // Ensure username exists.
}

/// Sends the chat history to the client.
fn send_chat_history(
    stream: &mut TcpStream,                       // The client's TCP stream.
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>, // Shared chat history.
) -> ChatResult<()> {
    let history = chat_history.read()?; // Acquire a read lock to access the chat history.
    for msg in history.iter() {
        send_message_to_client(stream, msg)?; // Send each message in the history to the client.
    }
    Ok(())
}

/// Broadcasts a "join" message to all clients.
fn broadcast_join_message(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>, // Shared clients map.
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,  // Shared usernames map.
    peer_addr: SocketAddr,                                 // The address of the client joining.
    username: &str,                                        // The username of the client joining.
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,          // Shared chat history.
) -> ChatResult<()> {
    // Add the username to the shared usernames map.
    {
        let mut usernames_lock = usernames.write()?;
        usernames_lock.insert(peer_addr, username.to_string());
    }

    // Broadcast a system "join" message to all clients.
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

/// Broadcasts a system message to all clients.
fn broadcast_system_message(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>, // Shared clients map.
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>,          // Shared chat history.
    sender: SocketAddr,                                    // The sender's address.
    username: &str,                                        // The sender's username.
    message_type: ChatMessageType, // The type of message (e.g., join, leave).
    content: String,               // The message content.
) -> ChatResult<ChatMessage> {
    let msg = ChatMessage {
        message_type,                         // Type of the system message.
        username: Some(username.to_string()), // Include the sender's username.
        content,                              // Include the message content.
    };
    broadcast_message(clients, sender, &msg, chat_history); // Broadcast the message to all clients.
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
    let mut buffer = [0; 1024]; // Buffer to store incoming data.
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Connection closed by the client.
            Ok(n) => {
                let raw_msg = String::from_utf8_lossy(&buffer[..n]).trim().to_string(); // Convert bytes to string.
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
                    eprintln!("Failed to parse message: {}", raw_msg); // Log parsing error.
                }
            }
            Err(_) => break, // Exit loop on read error.
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
            // Broadcast a regular chat message.
            let msg = ChatMessage {
                message_type: ChatMessageType::Message,
                username: Some(username.to_string()),
                content: chat_msg.content,
            };
            broadcast_message(clients, peer_addr, &msg, chat_history);
        }
        ChatMessageType::Command(CommandType::List) => {
            // Respond to a `/list` command with a list of online users.
            send_user_list(stream, usernames)?;
        }
        ChatMessageType::Command(CommandType::Quit) | ChatMessageType::Leave => {
            // Handle client disconnection for `/quit` or leave message.
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
            eprintln!("Unhandled message type: {:?}", chat_msg.message_type); // Log unsupported message type.
        }
    }
    Ok(())
}

/// Sends the list of online users to the client.
fn send_user_list(
    stream: &mut TcpStream, // The client's TCP stream.
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>, // Shared map of usernames.
) -> ChatResult<()> {
    let users = {
        // Acquire a read lock on the usernames hashmap and collect all usernames.
        let usernames_lock = usernames.read()?;
        if usernames_lock.is_empty() {
            // Debugging log if no users are found.
            eprintln!("DEBUG: No users found in usernames map.");
        } else {
            // Log the usernames currently online for debugging purposes.
            println!(
                "DEBUG: Found users in usernames map: {:?}",
                usernames_lock.values().cloned().collect::<Vec<_>>()
            );
        }
        // Collect all usernames into a vector.
        usernames_lock.values().cloned().collect::<Vec<_>>()
    };

    // Create a chat message containing the list of online users.
    let list_msg = ChatMessage {
        message_type: ChatMessageType::Command(CommandType::List), // Indicates a `/list` command response.
        username: None, // No specific sender for this system message.
        content: if users.is_empty() {
            "No users online.".to_string() // Message for when no users are online.
        } else {
            format!("Online users: {}", users.join(", ")) // Format the usernames as a comma-separated string.
        },
    };

    // Send the message back to the client who requested the user list.
    send_message_to_client(stream, &list_msg)
}

/// Handles client disconnects by broadcasting a "leave" message and cleaning up.
fn handle_client_disconnect(
    stream: &mut TcpStream, // The disconnecting client's TCP stream.
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>, // Shared map of connected clients.
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>, // Shared map of usernames.
    peer_addr: SocketAddr,  // The address of the disconnecting client.
    username: &str,         // The username of the disconnecting client.
    message_type: &ChatMessageType, // The type of message indicating the disconnect.
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>, // Shared chat history.
) -> ChatResult<()> {
    // Broadcast a "leave" system message to all other clients.
    let leave_msg = broadcast_system_message(
        clients,
        chat_history,
        peer_addr,
        username,
        message_type.clone(), // Message type (e.g., leave or quit).
        format!("{} has left the chat", username), // Content of the "leave" message.
    )?;

    // Send the "leave" message to the disconnecting client.
    send_message_to_client(stream, &leave_msg)?;

    // Remove the client from shared state (clients map and usernames map).
    {
        let mut clients_lock = clients.write()?; // Acquire a write lock on the clients map.
        clients_lock.remove(&peer_addr); // Remove the client.

        let mut usernames_lock = usernames.write()?; // Acquire a write lock on the usernames map.
        usernames_lock.remove(&peer_addr); // Remove the username.
    }

    Ok(())
}

/// Removes a client from the shared state after disconnection.
fn cleanup_client(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>, // Shared map of connected clients.
    usernames: &Arc<RwLock<HashMap<SocketAddr, String>>>,  // Shared map of usernames.
    peer_addr: SocketAddr,                                 // The address of the client to remove.
) {
    // Remove the client from the clients map.
    clients.write().ok().map(|mut lock| lock.remove(&peer_addr));
    // Remove the client's username from the usernames map.
    usernames
        .write()
        .ok()
        .map(|mut lock| lock.remove(&peer_addr));
}

/// Sends a message to a single client.
fn send_message_to_client(
    stream: &mut TcpStream, // The client's TCP stream.
    message: &ChatMessage,  // The message to send.
) -> ChatResult<()> {
    // Serialize the chat message to JSON format.
    let serialized_msg = serde_json::to_string(message)?;
    // Write the serialized message to the client's stream, followed by a newline.
    stream.write_all(format!("{}\n", serialized_msg).as_bytes())?;
    Ok(())
}

/// Broadcasts a message to all clients except the sender and updates the chat history.
fn broadcast_message(
    clients: &Arc<RwLock<HashMap<SocketAddr, TcpStream>>>, // Shared map of connected clients.
    sender: SocketAddr, // The address of the sender (to exclude from broadcasting).
    message: &ChatMessage, // The message to broadcast.
    chat_history: &Arc<RwLock<Vec<ChatMessage>>>, // Shared chat history.
) {
    // Add the message to the shared chat history.
    {
        let mut history_lock = chat_history.write().unwrap();
        history_lock.push(message.clone());
    }

    // Serialize the message for transmission.
    let serialized = serde_json::to_string(message).unwrap_or_default();
    let mut failed_clients = vec![]; // List to track clients that fail to receive the message.

    // Use a read lock to access the clients map for broadcasting.
    {
        let clients_lock = clients.read().unwrap();
        for (&addr, client) in clients_lock.iter() {
            if addr != sender {
                // Skip the sender.
                if let Ok(mut writable_client) = client.try_clone() {
                    if let Err(_) =
                        writable_client.write_all(format!("{}\n", serialized).as_bytes())
                    {
                        failed_clients.push(addr); // Add failed clients to the list.
                    }
                } else {
                    failed_clients.push(addr); // Add clients that failed to clone.
                }
            }
        }
    }

    // Remove any clients that failed during broadcasting.
    if !failed_clients.is_empty() {
        let mut clients_lock = clients.write().unwrap();
        for addr in failed_clients {
            eprintln!("Removing failed client: {}", addr);
            clients_lock.remove(&addr);
        }
    }
}
