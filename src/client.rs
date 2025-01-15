use std::io::{self, BufRead, Write};
use std::net::TcpStream;
use std::thread;
use std::env;

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
                    // Directly print the message as it comes from the server
                    print!("\r{}\n[You]: ", msg);
                    io::stdout().flush().unwrap(); // Ensure prompt is displayed properly
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
        if msg == "/quit" {
            // Send a disconnect message to the server and break the loop
            stream.write_all(format!("{} has left the chat\n", username).as_bytes())?;
            println!("You have disconnected from the chat.");
            break;
        }
        // Send the message to the server
        stream.write_all(format!("{}\n", msg).as_bytes())?;

        // Display your own message locally without extra newline
        print!("\r[You]: {}", msg);
        print!("\n[You]: ");
        io::stdout().flush().unwrap(); // Flush to display the prompt after each input
    }

    Ok(())
}
