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

    // Clone the stream to read messages from the server
    let mut stream_clone = stream.try_clone()?;

    // Spawn a thread to handle incoming messages from the server
    thread::spawn(move || {
        let reader = io::BufReader::new(&mut stream_clone);
        for line in reader.lines() {
            match line {
                Ok(msg) => println!("> {}", msg),
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
        stream.write_all(msg.as_bytes())?;
        stream.write_all(b"\n")?;
        print!("[You]: ");
        io::stdout().flush().unwrap(); // Flush to display the prompt after each input
    }

    Ok(())
}
