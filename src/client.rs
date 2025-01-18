// src/client.rs
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, BufRead, Write};
use std::net::TcpStream;
use std::thread;

/// A message structure matching what the server expects
#[derive(Serialize, Deserialize, Debug)]
struct ChatMessage {
    message_type: String,
    username: Option<String>,
    content: String,
}

fn main() -> std::io::Result<()> {
    // Read port from CLI arguments or default to 8081
    let args: Vec<String> = env::args().collect();
    let port = if args.len() > 1 { &args[1] } else { "8081" };

    // Connect to server
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))?;
    println!("Connected to the server!");

    // Prompt user for a username
    print!("Enter your username: ");
    io::stdout().flush().unwrap();
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    // Send that username to server
    stream.write_all(format!("{}\n", username).as_bytes())?;

    // Clone the stream so we can read in a separate thread
    let stream_clone = stream.try_clone()?;

    // Spawn a thread for incoming messages
    thread::spawn(move || {
        let reader = io::BufReader::new(stream_clone);
        for line in reader.lines() {
            match line {
                Ok(raw) => {
                    let trimmed = raw.trim();
                    match serde_json::from_str::<ChatMessage>(trimmed) {
                        Ok(msg) => match msg.message_type.as_str() {
                            "message" => {
                                if let Some(u) = msg.username {
                                    println!("\r[{}]: {}", u, msg.content);
                                }
                            }
                            "join" | "leave" | "list" => {
                                println!("\r{}", msg.content);
                            }
                            other => {
                                println!("\rUnknown message type: {}", other);
                            }
                        },
                        Err(e) => {
                            eprintln!("\rFailed to parse JSON: {} (raw = {})", e, trimmed);
                        }
                    }
                    // Reprompt
                    print!("[You]: ");
                    io::stdout().flush().unwrap();
                }
                Err(e) => {
                    eprintln!("Error reading server message: {}", e);
                    break;
                }
            }
        }
    });

    // Main loop: read from stdin and send
    let stdin = io::stdin();
    print!("[You]: ");
    io::stdout().flush().unwrap();
    for line in stdin.lock().lines() {
        let input = line?;
        // Check for commands
        if input == "/list" {
            stream.write_all(format!("{}\n", input).as_bytes())?;
            continue;
        }
        if input == "/quit" {
            let leave = ChatMessage {
                message_type: "leave".to_string(),
                username: Some(username.clone()),
                content: format!("{} has left the chat", username),
            };
            let serialized = serde_json::to_string(&leave).unwrap();
            stream.write_all(format!("{}\n", serialized).as_bytes())?;
            println!("You have disconnected.");
            break;
        }

        // Otherwise, a normal chat message
        let chat_msg = ChatMessage {
            message_type: "message".to_string(),
            username: Some(username.clone()),
            content: input.clone(),
        };
        let serialized = serde_json::to_string(&chat_msg).unwrap();
        stream.write_all(format!("{}\n", serialized).as_bytes())?;

        // Print local echo
        print!("\r[You]: {}\n[You]: ", input);
        io::stdout().flush().unwrap();
    }

    Ok(())
}
