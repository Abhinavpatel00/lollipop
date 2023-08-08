use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const BUFFER_SIZE: usize = 8192;
const MAX_THREADS: usize = 8;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to bind");
    println!("Server is running at http://127.0.0.1:8080");

    let thread_counter = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        while thread_counter.load(Ordering::Relaxed) >= MAX_THREADS {
            // Wait if too many threads are active
            std::thread::yield_now();
        }

        let thread_counter_clone = thread_counter.clone();
        thread::spawn(move || {
            let _guard = ThreadGuard::new(&thread_counter_clone);

            if let Err(e) = handle_connection(stream.unwrap()) {
                eprintln!("Error handling connection: {}", e);
            }
        });
    }
}

struct ThreadGuard {
    counter: Arc<AtomicUsize>,
}

impl ThreadGuard {
    fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self {
            counter: counter.clone(),
        }
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

fn handle_connection(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; BUFFER_SIZE];
    let mut total_read = 0;

    loop {
        let bytes_read = stream.read(&mut buffer[total_read..])?;
        if bytes_read == 0 {
            break;
        }
        total_read += bytes_read;

        // Break if the buffer is full to avoid overflows
        if total_read >= buffer.len() {
            break;
        }
    }

    let request = String::from_utf8_lossy(&buffer[..total_read]);
    let response = match request.lines().next() {
        Some(line) if line.starts_with("GET / ") => read_file("public/index.html"),
        Some(line) if line.starts_with("GET /about ") => {
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><head><title>About</title></head><body>\
            <h1>About Us</h1><p>This server is powered by Rust.</p>\
            </body></html>"
                .to_owned()
        }
        Some(line) => {
            let file_name = line.split_whitespace().nth(1).unwrap_or("/");
            read_file(&format!("public{}", file_name))
        }
        None => "HTTP/1.1 500 Internal Server Error\r\n\r\n500 Internal Server Error".to_owned(),
    };

    let response_bytes = response.as_bytes();
    let mut written = 0;

    while written < response_bytes.len() {
        match stream.write(&response_bytes[written..]) {
            Ok(0) => break,
            Ok(n) => written += n,
            Err(_) => break,
        }
    }

    stream.flush()?;
    Ok(())
}

fn read_file(file_path: &str) -> String {
    match fs::read(file_path) {
        Ok(content) => {
            let content_type = if file_path.ends_with(".css") {
                "text/css"
            } else if file_path.ends_with(".js") {
                "application/javascript"
            } else {
                "text/html"
            };

            let response_header = format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\n\r\n", content_type);
            let mut response = Vec::with_capacity(response_header.len() + content.len());
            response.extend_from_slice(response_header.as_bytes());
            response.extend_from_slice(&content);

            String::from_utf8_lossy(&response).to_string()
        }
        Err(_) => read_file("public/404.html"),
    }
}
