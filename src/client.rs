mod message;
use crate::message::{ChatMessage, ChatMessageType, CommandType};
use std::env;
use std::io::{self, BufRead, Write};
use std::net::TcpStream;
use std::thread;

/// Main entry point for the client application
fn main() -> std::io::Result<()> {
    let port = env::args().nth(1).unwrap_or_else(|| "8081".to_string());
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))?;
    println!("Connected to the server!");

    let username = prompt_for_username()?;
    send_join_message(&mut stream, &username)?;

    // Handle incoming messages in a separate thread
    let stream_clone = stream.try_clone()?;
    thread::spawn(move || handle_incoming_messages(stream_clone));

    // Handle user input in the main thread
    handle_user_input(&mut stream, &username)
}

/// Prompts the user for their username
fn prompt_for_username() -> std::io::Result<String> {
    print!("Enter your username: ");
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    Ok(username.trim().to_string())
}

/// Sends a "join" message to the server
fn send_join_message(stream: &mut TcpStream, username: &str) -> std::io::Result<()> {
    let join_msg = ChatMessage {
        message_type: ChatMessageType::Join,
        username: Some(username.to_string()),
        content: format!("{} has joined the chat", username),
    };
    send_message(stream, &join_msg)
}

/// Handles user input and sends messages/commands to the server
fn handle_user_input(stream: &mut TcpStream, username: &str) -> std::io::Result<()> {
    let stdin = io::stdin();
    print!("[You]: ");
    io::stdout().flush()?;
    for line in stdin.lock().lines() {
        let input = line?;
        let chat_msg = parse_user_input(&input, username);

        send_message(stream, &chat_msg)?;

        if let ChatMessageType::Command(CommandType::Quit) = chat_msg.message_type {
            println!("You have disconnected from the chat.");
            break;
        }

        if let ChatMessageType::Message = chat_msg.message_type {
            print!("\r[You]: {}\n[You]: ", input);
            io::stdout().flush()?;
        }
    }
    Ok(())
}

/// Handles incoming messages from the server
fn handle_incoming_messages(stream: TcpStream) {
    let reader = io::BufReader::new(stream);
    for line in reader.lines() {
        match line {
            Ok(msg) => {
                if let Ok(chat_msg) = serde_json::from_str::<ChatMessage>(&msg) {
                    display_message(chat_msg);
                } else {
                    eprintln!("\rFailed to parse message: {}", msg);
                }
                print!("[You]: ");
                io::stdout().flush().unwrap();
            }
            Err(e) => {
                eprintln!("Error reading message: {}", e);
                break;
            }
        }
    }
}

/// Sends a `ChatMessage` to the server
fn send_message(stream: &mut TcpStream, message: &ChatMessage) -> std::io::Result<()> {
    let serialized_msg = serde_json::to_string(message)?;
    stream.write_all(format!("{}\n", serialized_msg).as_bytes())
}

/// Displays a `ChatMessage` based on its type
fn display_message(chat_msg: ChatMessage) {
    match chat_msg.message_type {
        ChatMessageType::Message => {
            if let Some(username) = chat_msg.username {
                println!("\r[{}]: {}", username, chat_msg.content);
            }
        }
        ChatMessageType::Join | ChatMessageType::Leave => {
            println!("\r{}", chat_msg.content);
        }
        ChatMessageType::Command(CommandType::List) => {
            println!("\r{}", chat_msg.content);
        }
        ChatMessageType::Command(CommandType::Quit) => {
            if let Some(username) = chat_msg.username {
                println!("\r{} has left the chat.", username);
            }
        }
    }
}

/// Parses user input into a structured `ChatMessage`
fn parse_user_input(input: &str, username: &str) -> ChatMessage {
    if let Some(command) = match input {
        "/list" => Some(CommandType::List),
        "/quit" => Some(CommandType::Quit),
        _ => None,
    } {
        ChatMessage {
            message_type: ChatMessageType::Command(command.clone()), // Clone `command` here
            username: if command == CommandType::Quit {
                Some(username.to_string())
            } else {
                None
            },
            content: String::new(),
        }
    } else {
        ChatMessage {
            message_type: ChatMessageType::Message,
            username: Some(username.to_string()),
            content: input.to_string(),
        }
    }
}
