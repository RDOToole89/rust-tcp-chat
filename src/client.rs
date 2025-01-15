use std::io::{self, BufRead, Write};
use std::net::TcpStream;
use std::thread;

fn main() -> std::io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:9000")?;
    println!("Connected to the server!");

    let mut stream_clone = stream.try_clone()?;
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

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let msg = line?;
        stream.write_all(msg.as_bytes())?;
    }

    Ok(())
}
