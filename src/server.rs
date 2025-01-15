use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

// Function to handle communication with a connected client
fn handle_client(mut stream: TcpStream, clients: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>) {
    let peer_addr = stream.peer_addr().unwrap(); // Get the client's socket address
    println!("Started handling client: {:?}", peer_addr);

    let mut buffer = [0; 1024]; // Fixed-size buffer for reading data
    let mut leftover = Vec::new(); // To handle partial messages across multiple reads

    loop {
        // Read bytes from the stream into the buffer
        match stream.read(&mut buffer) {
            Ok(0) => {
                // 0 bytes read means the client has disconnected
                println!("Client disconnected: {:?}", peer_addr);
                break;
            }
            Ok(n) => {
                // Successfully read 'n' bytes into the buffer
                println!("Read {} bytes from {:?}", n, peer_addr);

                // Append the new data to any leftover data from previous reads
                leftover.extend_from_slice(&buffer[..n]);

                // Split the data by newline to handle complete messages
                let mut messages = Vec::new();
                let mut last_index = 0;

                for (i, &byte) in leftover.iter().enumerate() {
                    if byte == b'\n' {
                        let msg = String::from_utf8_lossy(&leftover[last_index..i]).to_string();
                        messages.push(msg);
                        last_index = i + 1;
                    }
                }

                // Remove processed data from the leftover buffer
                leftover.drain(..last_index);

                // Lock the clients list to get mutable access
                let mut clients = clients.lock().unwrap();

                // Send each complete message to all other clients
                for msg in messages {
                    println!("Received message from {:?}: {}", peer_addr, msg);
                    for (addr, client) in clients.iter_mut() {
                        if *addr != peer_addr {
                            println!("Sending message to {}", addr);
                            client.write_all(msg.as_bytes()).unwrap();
                            client.write_all(b"\n").unwrap();
                        }
                    }
                }
            }
            Err(e) => {
                // Handle any errors during reading
                println!("Failed to read from client {:?}: {}", peer_addr, e);
                break;
            }
        }
    }

    // Remove the disconnected client from the list
    clients.lock().unwrap().remove(&peer_addr);
    println!("Client disconnected: {:?}", peer_addr);
}

    
fn main() -> std::io::Result<()> {
    let mut port = 8081; // Starting port number
    let listener = loop {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(listener) => {
                println!("Server listening on 127.0.0.1:{}", port);
                break listener; // Break the loop if binding succeeds
            }
            Err(_) => {
                println!("Port {} is in use, trying port {}", port, port + 1);
                port += 1; // Try the next port
            }
        }
    };

    let clients = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer_addr = stream.peer_addr().unwrap(); // Get the client's socket address
                println!("New client connected: {:?}", peer_addr);
  
                let clients = Arc::clone(&clients);
                clients.lock().unwrap().insert(peer_addr, stream.try_clone().unwrap());

                // Spawn a new thread to handle the client
                thread::spawn(move || handle_client(stream, clients));
            }
            Err(e) => {
                println!("Failed to accept connection: {}", e);
            }
        }
    }

    Ok(())
}
