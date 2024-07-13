use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

#[allow(dead_code)]
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

#[allow(dead_code)]
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

struct Request<'a> {
    method: &'a str,
    path: &'a str,
    _version: &'a str,
    headers: HashMap<String, &'a str>,
    body: &'a str,
}

impl<'a> Request<'a> {
    fn from_str(s: &'a str) -> Self {
        let (status_line, rest) = s.split_once("\r\n").unwrap();

        let [method, path, _version]: [&str; 3] = status_line
            .split_whitespace()
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let (string_headers, body) = rest.split_once("\r\n\r\n").unwrap_or_default();
        let headers = string_headers
            .split("\r\n")
            .map(|line| {
                line.split_once(": ")
                    .map(|(h, c)| (h.to_lowercase(), c))
                    .unwrap()
            })
            .collect::<HashMap<_, _>>();

        Request {
            method,
            path,
            _version,
            headers,
            body,
        }
    }
}

fn main() {
    let mut args = std::env::args();

    let mut directory_path = PathBuf::new();
    while let Some(arg) = args.next() {
        match arg.as_ref() {
            "--directory" => {
                let path = args.next().unwrap_or_default();
                directory_path = PathBuf::from(path);
            }
            _ => continue,
        }
    }

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let pool = ThreadPool::new(10);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");

                let directory_path = directory_path.clone();
                pool.execute(|| {
                    handle_connection(stream, directory_path);
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream, files_directory: PathBuf) {
    let mut buffer = [0; 256];
    let _n_read = stream.read(&mut buffer).unwrap();

    let request_str = std::str::from_utf8(&buffer).unwrap();
    let request = Request::from_str(request_str);

    let response = match request.path {
        "/" => "HTTP/1.1 200 OK\r\n\r\n".to_string(),
        "/user-agent" => handle_get_user_agent(&request),
        p if p.starts_with("/files/") => {
            let relative_path = p.strip_prefix("/files/").unwrap();
            match request.method {
                "GET" => handle_get_files(&request, relative_path, files_directory),
                "POST" => handle_post_files(&request, relative_path, files_directory),
                _ => unimplemented!(),
            }
        }
        p if p.starts_with("/echo/") => handle_get_echo(&request),
        _ => "HTTP/1.1 404 Not Found\r\n\r\n".to_string(),
    };

    stream.write_all(response.as_bytes()).unwrap();
}

fn handle_get_user_agent(request: &Request) -> String {
    let user_agent = request.headers.get("user-agent").unwrap_or(&"");
    let len = user_agent.len();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{user_agent}"
    )
}

fn handle_get_echo(request: &Request) -> String {
    let str = request.path.strip_prefix("/echo/").unwrap();
    let len = str.len();

    format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{str}")
}

fn handle_get_files(_request: &Request, filename: &str, files_directory: PathBuf) -> String {
    if let Ok(mut file) = File::open(files_directory.join(filename)) {
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n{contents}",
            contents.len()
        )
    } else {
        "HTTP/1.1 404 Not Found\r\n\r\n".to_string()
    }
}

fn handle_post_files(request: &Request, filename: &str, files_directory: PathBuf) -> String {
    let mut file = File::create(files_directory.join(filename)).unwrap();
    file.write_all(request.body.as_bytes()).unwrap();

    "HTTP/1.1 201 Created\r\n\r\n".to_string()
}
