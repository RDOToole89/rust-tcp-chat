use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

// Function to handle communication with a connected client
fn handle_client(mut stream: TcpStream, clients: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>, usernames: Arc<Mutex<HashMap<SocketAddr, String>>>) {
    let peer_addr = stream.peer_addr().unwrap();
    println!("Started handling client: {:?}", peer_addr);

    let mut buffer = [0; 1024];
    
    // Read the initial message to get the username
    match stream.read(&mut buffer) {
        Ok(n) if n > 0 => {
            let username = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
            usernames.lock().unwrap().insert(peer_addr, username.clone());
            println!("Username for {:?} is {}", peer_addr, username);

            // Notify all clients that a new user has joined
            let join_msg = format!("{} has entered the chat\n", username);
            broadcast_message(&clients, &peer_addr, &join_msg);
        }
        _ => {
            println!("Failed to read username from {:?}", peer_addr);
            return;
        }
    }

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                println!("Client disconnected: {:?}", peer_addr);
                break;
            }
            Ok(n) => {
                let msg = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                if msg.is_empty() {
                    continue;
                }

                let usernames = usernames.lock().unwrap();
                let sender_username = usernames
                .get(&peer_addr)
                .map(|s| s.as_str()) // Convert `String` to `&str`
                .unwrap_or("Unknown");
            
                let formatted_msg = format!("[{}]: {}\n", sender_username, msg);

                broadcast_message(&clients, &peer_addr, &formatted_msg);
            }
            Err(e) => {
                println!("Failed to read from client {:?}: {}", peer_addr, e);
                break;
            }
        }
    }

    // Notify all clients that the user has left
    let leave_msg = format!("{} has left the chat\n", usernames.lock().unwrap().get(&peer_addr).unwrap());
    broadcast_message(&clients, &peer_addr, &leave_msg);

    clients.lock().unwrap().remove(&peer_addr);
    usernames.lock().unwrap().remove(&peer_addr);
    println!("Client disconnected: {:?}", peer_addr);
}

fn broadcast_message(clients: &Arc<Mutex<HashMap<SocketAddr, TcpStream>>>, sender: &SocketAddr, message: &str) {
    let mut clients = clients.lock().unwrap();
    for (addr, client) in clients.iter_mut() {
        if addr != sender {
            client.write_all(message.as_bytes()).unwrap();
        }
    }
}

    
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

    let clients = Arc::new(Mutex::new(HashMap::new()));
    let usernames = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer_addr = stream.peer_addr().unwrap();
                println!("New client connected: {:?}", peer_addr);

                let clients = Arc::clone(&clients);
                let usernames = Arc::clone(&usernames);
                clients.lock().unwrap().insert(peer_addr, stream.try_clone().unwrap());

                thread::spawn(move || handle_client(stream, clients, usernames));
            }
            Err(e) => {
                println!("Failed to accept connection: {}", e);
            }
        }
    }

    Ok(())
}
