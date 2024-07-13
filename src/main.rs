use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<Job>,
}

impl ThreadPool {
    fn new(size: usize) -> Self {
        let mut workers = Vec::with_capacity(size);
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        for i in 0..size {
            workers.push(Worker::new(i, receiver.clone()))
        }

        Self { workers, sender }
    }

    fn execute<F: FnOnce() + Send + 'static>(&self, function: F) {
        let job = Box::new(function);
        self.sender.send(job).unwrap();
    }
}

struct Worker {
    id: usize,
    handle: std::thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Job>>>) -> Self {
        let handle = thread::spawn(move || loop {
            let job = receiver.lock().unwrap().recv().unwrap();
            println!("Worker {id} got a job; executing.");

            job();
        });
        Self { id, handle }
    }
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let pool = ThreadPool::new(10);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");

                pool.execute(|| {
                    handle_connection(stream);
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 256];
    let _n_read = stream.read(&mut buffer).unwrap();

    let request = std::str::from_utf8(&buffer).unwrap();
    let (status_line, rest) = request.split_once("\r\n").unwrap();

    let [_method, path, _version]: [&str; 3] = status_line
        .split_whitespace()
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let mut headers: HashMap<String, &str> = HashMap::new();
    if let Some((string_headers, _body)) = rest.split_once("\r\n\r\n") {
        headers = string_headers
            .split("\r\n")
            .map(|line| {
                line.split_once(": ")
                    .map(|(h, c)| (h.to_lowercase(), c))
                    .unwrap()
            })
            .collect::<HashMap<_, _>>();
    }

    let response = match path {
        "/" => "HTTP/1.1 200 OK\r\n\r\n".to_string(),
        "/user-agent" => {
            let user_agent = headers.get("user-agent").unwrap_or(&"");
            let len = user_agent.len();
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{user_agent}"
            )
        }
        p if p.starts_with("/echo/") => {
            let str = p.strip_prefix("/echo/").unwrap();
            let len = str.len();

            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{str}"
            )
        }
        _ => "HTTP/1.1 404 Not Found\r\n\r\n".to_string(),
    };

    stream.write_all(response.as_bytes()).unwrap();
}
