// Module imports
mod message; // Import the `message` module for ChatMessage and related types.
use crate::message::{ChatMessage, ChatMessageType, CommandType}; // Re-export specific types from the module.
use std::env; // For accessing command-line arguments.
use std::io::{self, BufRead, Write}; // For handling input/output operations.
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
// For managing TCP connections.
use std::thread; // For spawning threads to handle parallel tasks.

/// Main entry point for the client application.
fn main() -> std::io::Result<()> {
    const DEFAULT_PORT: &str = "8081";
    // Retrieve the port from command-line arguments, or default to "8081" if none is provided.
    let port = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    // Create a connection to the server using `TcpStream`.
    // The `?` operator propagates errors to the caller (here it uses `std::io::Result`).
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).map_err(|e| {
        log::error!("Failed to connect to server at 127.0.0.1:{}: {}", port, e);
        e
    })?;

    log::info!("Connected to the server!");

    // Prompt the user to input their username and send a "join" message to the server.
    let username = prompt_for_username()?; // Call the function to get the username.
    send_join_message(&mut stream, &username)?; // Notify the server about the client joining.

    // Clone the stream to create a copy for the reader thread.
    // `try_clone()` duplicates the `TcpStream`, allowing it to be used in multiple threads.
    let stream_clone = stream.try_clone()?;
    let quit_flag = Arc::new(AtomicBool::new(false));
    let quit_flag_clone = quit_flag.clone();
    let handle = thread::spawn(move || handle_incoming_messages(stream_clone, quit_flag_clone));

    // Handle user input in the main thread.
    handle_user_input(&mut stream, &username, &quit_flag)?;

    // Wait for the reader thread to finish before exiting.
    if let Err(e) = handle.join() {
        log::error!("Failed to join thread: {:?}", e);
    }

    Ok(())
}

/// Prompts the user for their username.
fn prompt_for_username() -> std::io::Result<String> {
    print!("Enter your username: "); // Prompt message.
    io::stdout().flush()?; // Ensure the prompt is printed immediately by flushing the buffer.
    let mut username = String::new(); // Create a mutable `String` to store the username.
    io::stdin().read_line(&mut username)?; // Read input from the user.
    let username = username.trim().to_string(); // Remove trailing whitespace and return the username.

    // Validate username
    if username.is_empty() || username.len() > 20 {
        log::error!("Invalid username. It must be between 1 and 20 characters.");
        return prompt_for_username(); // Retry input.
    }

    Ok(username)
}

/// Sends a "join" message to the server.
fn send_join_message(stream: &mut TcpStream, username: &str) -> std::io::Result<()> {
    // Create a structured `ChatMessage` to indicate that the user has joined the chat.
    let join_msg = ChatMessage {
        message_type: ChatMessageType::Join, // Indicate a "join" message type.
        username: Some(username.to_string()), // Set the username.
        content: format!("{} has joined the chat", username), // Message content.
    };
    send_message(stream, &join_msg) // Use the `send_message` helper to send the message.
}

/// Handles user input from the terminal, sends messages or commands to the server,
/// and manages the client's quit state.
fn handle_user_input(
    stream: &mut TcpStream, // Mutable reference to the server connection (TCP stream).
    username: &str,         // Username of the client sending messages.
    quit_flag: &Arc<AtomicBool>, // Shared flag to signal when the client is quitting.
) -> std::io::Result<()> {
    let stdin = io::stdin(); // Access the standard input for reading user input.

    /// Prints the input prompt `[You]: ` to the terminal in a clean way.
    /// This function clears the current line (if any), moves the cursor to the beginning,
    /// and displays the prompt.
    fn print_prompt() -> std::io::Result<()> {
        // `\r`: Move cursor to the beginning of the current line.
        // `\x1B[2K`: ANSI escape sequence to clear the entire line.
        print!("\r\x1B[2K[You]: ");
        io::stdout().flush() // Flush the output buffer to ensure the prompt is displayed immediately.
    }

    print_prompt()?; // Display the initial prompt to the user.

    // Read input from the terminal in a loop, line by line.
    for line in stdin.lock().lines() {
        let input = line?; // Read a line of input and handle potential I/O errors.

        // Skip processing for empty input and redisplay the prompt.
        if input.trim().is_empty() {
            print_prompt()?; // Clear the line and show a clean prompt again.
            continue; // Skip to the next iteration of the loop.
        }

        // Parse the user's input into a structured `ChatMessage`.
        let chat_msg = parse_user_input(&input, username);

        // Attempt to send the parsed message to the server via the TCP stream.
        if let Err(e) = send_message(stream, &chat_msg) {
            eprintln!("Failed to send message: {}", e); // Log any errors while sending.
        }

        // Check if the user entered the `/quit` command.
        if matches!(
            chat_msg.message_type,
            ChatMessageType::Command(CommandType::Quit)
        ) {
            // Set the `quit_flag` to `true`, signaling other threads (e.g., the message handler) to exit.
            quit_flag.store(true, Ordering::SeqCst);
            println!("\rYou have disconnected from the chat."); // Inform the user of disconnection.
            break; // Exit the loop, ending the user input handling.
        }

        print_prompt()?; // Redisplay the prompt after processing the input.
    }

    Ok(()) // Indicate successful completion of the function.
}

/// Handles incoming messages from the server in a separate thread.
fn handle_incoming_messages(stream: TcpStream, quit_flag: Arc<AtomicBool>) {
    let reader = io::BufReader::new(stream);
    for line in reader.lines() {
        if quit_flag.load(Ordering::SeqCst) {
            break; // Exit if quit is signaled
        }

        match line {
            Ok(msg) => {
                if let Ok(chat_msg) = serde_json::from_str::<ChatMessage>(&msg) {
                    display_message(chat_msg);
                } else {
                    log::error!("Failed to parse message: {}", msg);
                }
                print!("[You]: ");
                if let Err(e) = io::stdout().flush() {
                    log::error!("Failed to flush stdout: {}", e);
                }
            }
            Err(e) => {
                log::error!("Error reading message: {}", e);
                break;
            }
        }
    }
}

/// Sends a `ChatMessage` to the server.
fn send_message(stream: &mut TcpStream, message: &ChatMessage) -> std::io::Result<()> {
    // Serialize the `ChatMessage` into JSON format.
    let serialized_msg = serde_json::to_string(message)?;
    // Write the serialized message to the TCP stream.
    stream.write_all(format!("{}\n", serialized_msg).as_bytes())
}

/// Displays a `ChatMessage` based on its type.
fn display_message(chat_msg: ChatMessage) {
    // Match the message type to determine how to display it.
    match chat_msg.message_type {
        ChatMessageType::Message => {
            if let Some(username) = chat_msg.username {
                println!("\r[{}]: {}", username, chat_msg.content); // Display regular messages with the sender's username.
            }
        }
        ChatMessageType::Join | ChatMessageType::Leave => {
            println!("\r{}", chat_msg.content); // Display join/leave system messages.
        }
        ChatMessageType::Command(CommandType::List) => {
            println!("\r{}", chat_msg.content); // Display the list of users.
        }
        ChatMessageType::Command(CommandType::Quit) => {
            if let Some(username) = chat_msg.username {
                println!("\r{} has left the chat.", username); // Display quit messages.
            }
        }
    }
}

/// Enum to represent user commands.
enum Command {
    List,
    Quit,
}

impl Command {
    /// Parse a string input into a `Command`.
    fn from_input(input: &str) -> Option<Self> {
        match input.trim() {
            "/list" => Some(Self::List),
            "/quit" => Some(Self::Quit),
            _ => None, // Return `None` for unrecognized commands.
        }
    }
}

/// Parses a command from user input.
fn parse_command(input: &str) -> Option<Command> {
    Command::from_input(input)
}

/// Parses user input into a structured `ChatMessage`.
fn parse_user_input(input: &str, username: &str) -> ChatMessage {
    // Check if the input is empty or contains only whitespace.
    if input.trim().is_empty() {
        return ChatMessage {
            message_type: ChatMessageType::Message, // Treat it as a regular message.
            username: Some(username.to_string()),   // Include the sender's username.
            content: "Empty input provided.".to_string(), // Set a default message.
        };
    }

    // Attempt to parse the input as a command.
    if let Some(command) = parse_command(input) {
        // Convert the `Command` into a `ChatMessage`.
        match command {
            Command::List => ChatMessage {
                message_type: ChatMessageType::Command(CommandType::List),
                username: None, // No username required for `/list`.
                content: String::new(),
            },
            Command::Quit => ChatMessage {
                message_type: ChatMessageType::Command(CommandType::Quit),
                username: Some(username.to_string()), // Include the username for `/quit`.
                content: String::new(),
            },
        }
    } else {
        // Fallback to a regular message if the input is not a command.
        ChatMessage {
            message_type: ChatMessageType::Message,
            username: Some(username.to_string()), // Include the sender's username.
            content: input.to_string(),           // Use the input as the message content.
        }
    }
}
