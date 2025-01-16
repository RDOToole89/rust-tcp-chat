use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, BufRead, Write};
use std::net::TcpStream;
use std::thread;

#[derive(Serialize, Deserialize, Debug)]
struct ChatMessage {
    message_type: String,     // "message", "join", or "leave"
    username: Option<String>, // Username of the sender (None for join/leave notifications)
    content: String,          // The actual message or notification content
}

fn main() -> std::io::Result<()> {
    // Read the port from command-line arguments
    let args: Vec<String> = env::args().collect();
    let port = if args.len() > 1 { &args[1] } else { "8081" };

    // Connect to the server
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))?;
    println!("Connected to the server!");

    // Prompt the user for a username
    let mut username = String::new();
    print!("Enter your username: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut username)?;
    username = username.trim().to_string(); // Remove newline characters

    // Send the username to the server
    stream.write_all(format!("{}\n", username).as_bytes())?;

    // Clone the stream to read messages from the server
    let stream_clone = stream.try_clone()?;

    // Spawn a thread to handle incoming messages from the server
    thread::spawn(move || {
        let reader = io::BufReader::new(stream_clone);
        for line in reader.lines() {
            match line {
                Ok(msg) => {
                    // Trim the incoming message to remove newlines or extra spaces
                    let trimmed_msg = msg.trim();

                    // Deserialize the incoming JSON message
                    let chat_msg: Result<ChatMessage, _> = serde_json::from_str(trimmed_msg);
                    match chat_msg {
                        Ok(chat_msg) => match chat_msg.message_type.as_str() {
                            "message" => {
                                if let Some(username) = chat_msg.username {
                                    println!("\r[{}]: {}", username, chat_msg.content);
                                }
                            }
                            "join" | "leave" | "list" => {
                                // Print out the content directly, e.g. “Bob has joined”, “Bob has left”,
                                // or “Online users: Alice, Bob, Charlie”
                                println!("\r{}", chat_msg.content);
                            }
                            _ => println!("\rUnknown message type: {}", chat_msg.message_type),
                        },
                        Err(e) => {
                            println!(
                                "\rFailed to parse message: {}. Raw message: {}",
                                e, trimmed_msg
                            );
                        }
                    }
                    // Reprint the `[You]:` prompt after processing the message
                    print!("[You]: ");
                    io::stdout().flush().unwrap();
                }
                Err(e) => {
                    println!("Error reading message: {}", e);
                    break;
                }
            }
        }
    });

    // Main thread handles sending messages to the server
    let stdin = io::stdin();
    print!("[You]: ");
    io::stdout().flush().unwrap(); // Ensure the prompt is displayed immediately
    for line in stdin.lock().lines() {
        let msg = line?;

        if msg == "/list" {
            // Send the "/list" command to the server
            stream.write_all(format!("{}\n", msg).as_bytes())?;
            continue; // Skip local echoing
        }

        if msg == "/quit" {
            // Send a disconnect message to the server and break the loop
            let leave_msg = ChatMessage {
                message_type: "leave".to_string(),
                username: Some(username.clone()),
                content: format!("{} has left the chat", username),
            };
            let serialized_msg = serde_json::to_string(&leave_msg).unwrap();
            stream.write_all(format!("{}\n", serialized_msg).as_bytes())?;
            println!("You have disconnected from the chat.");
            break;
        }
        // Create and send the chat message as JSON
        let chat_msg = ChatMessage {
            message_type: "message".to_string(),
            username: Some(username.clone()),
            content: msg.clone(),
        };
        let serialized_msg = serde_json::to_string(&chat_msg).unwrap();
        stream.write_all(format!("{}\n", serialized_msg).as_bytes())?;

        // Display your own message locally without extra newline
        print!("\r[You]: {}\n[You]: ", msg);
        io::stdout().flush().unwrap(); // Flush to display the prompt after each input
    }

    Ok(())
}
