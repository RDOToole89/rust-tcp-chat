use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

// Function to handle communication with a connected client
fn handle_client(mut stream: TcpStream, clients: Arc<Mutex<HashMap<String, TcpStream>>>) {
    let mut buffer = [0; 1024]; // Buffer to store data from the client

    println!("Started handling client: {:?}", stream.peer_addr().unwrap());

    loop {
        // Try to read data from the client into the buffer
        match stream.read(&mut buffer) {
            Ok(0) => {
                // Client has disconnected (0 bytes read means connection closed)
                println!("Client disconnected: {:?}", stream.peer_addr().unwrap());
                break;
            }
            Ok(n) => {
                // Successfully read 'n' bytes from the client
                let msg = String::from_utf8_lossy(&buffer[..n]);
                println!("Received message from {:?}: {}", stream.peer_addr().unwrap(), msg);

                // Lock the clients list to get mutable access
                let mut clients = clients.lock().unwrap();

                // Iterate over all connected clients and send the message to them
                for (_, client) in clients.iter_mut() {
                    if client.peer_addr().unwrap() != stream.peer_addr().unwrap() {
                        println!("Sending message to {:?}", client.peer_addr().unwrap());
                        client.write_all(msg.as_bytes()).unwrap();
                    }
                }
            }
            Err(e) => {
                // An error occurred while reading from the client
                println!("Failed to read from client {:?}: {}", stream.peer_addr().unwrap(), e);
                break;
            }
        }
    }
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
                println!("New client connected: {:?}", stream.peer_addr());
  
                let peer_addr = stream.peer_addr().unwrap().to_string();
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
