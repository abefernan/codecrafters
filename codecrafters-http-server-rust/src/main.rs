use std::collections::HashMap;
use std::io::{prelude::*, BufReader};
use std::net::{TcpListener, TcpStream};
use std::{fs, thread};

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn main() {
    start_server("127.0.0.1:4221");
}

fn start_server(socket: &str) {
    let listener =
        TcpListener::bind(socket).expect("Listener could not be bound to socket: {socket}");

    for res_stream in listener.incoming() {
        match res_stream {
            Ok(stream) => thread::spawn(|| handle_stream(stream)),
            _ => continue,
        };
    }
}

fn handle_stream(stream: TcpStream) {
    let req = parse_req(&stream);
    route_req(stream, req);
}

fn parse_req(stream: &TcpStream) -> HttpRequest {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap_or(0);

    let split_start_line = line.split_whitespace().take(2).collect::<Vec<&str>>();
    let [method, path] = match split_start_line[..] {
        [str_method, str_path] => [str_method.to_string(), str_path.to_string()],
        _ => [String::new(), String::new()],
    };

    let mut headers: HashMap<String, String> = HashMap::new();

    loop {
        line.clear();
        reader.read_line(&mut line).unwrap_or(0);

        if line.trim().is_empty() {
            break;
        }

        let res_header = line.split_once(": ");

        if let Some((key, value)) = res_header {
            headers.insert(key.to_lowercase(), trim_newlines(value));
        }
    }

    let opt_content_length = headers.get("content-length");

    let mut body: Vec<u8> = match opt_content_length {
        Some(str_content_length) => {
            let num_content_length = str_content_length.parse::<usize>().unwrap_or(0);
            vec![0; num_content_length]
        }
        _ => vec![0; 0],
    };

    reader.read_exact(&mut body).unwrap_or(());

    HttpRequest {
        method,
        path,
        headers,
        body,
    }
}

fn route_req(stream: TcpStream, req: HttpRequest) {
    if req.path == "/" {
        do_ok(stream);
    } else if req.path.starts_with("/echo/") {
        do_echo(stream, req);
    } else if req.path.starts_with("/files/") {
        do_route_files(stream, req);
    } else if req.path.starts_with("/") && !&req.path[1..].contains("/") {
        do_get_header(stream, req);
    } else {
        try_write_404(stream);
    }
}

fn do_ok(stream: TcpStream) {
    try_write(stream, "HTTP/1.1 200 OK\r\n\r\n");
}

fn do_get_header(stream: TcpStream, req: HttpRequest) {
    let queried_key = req.path.split("/").nth(1).unwrap_or("");
    let queried_value_option = req.headers.get(queried_key);

    match queried_value_option {
        Some(queried_value) => {
            try_write(
                stream,
                format!(
                    "HTTP/1.1 200 OK\r\n\
                        Content-Type: text/plain\r\n\
                        Content-Length: {}\r\n\r\n\
                        {queried_value}\r\n\r\n",
                    queried_value.len(),
                )
                .as_str(),
            );
        }
        _ => {
            try_write_404(stream);
        }
    }
}

fn do_echo(stream: TcpStream, req: HttpRequest) {
    let (_, a_random_string) = req.path.split_once("/echo/").unwrap_or(("", ""));

    try_write(
        stream,
        format!(
            "HTTP/1.1 200 OK\r\n\
                Content-Type: text/plain\r\n\
                Content-Length: {}\r\n\r\n\
                {a_random_string}\r\n\r\n",
            a_random_string.len()
        )
        .as_str(),
    );
}

fn do_route_files(stream: TcpStream, req: HttpRequest) {
    let directory = std::env::args().nth(2).expect("no directory provided");
    let path = req.path.clone();
    let (_, queried_file) = path.split_once("/files/").unwrap_or(("", ""));

    if req.method == "GET" {
        do_get_file(stream, directory, queried_file);
    } else if req.method == "POST" {
        do_write_file(stream, req, directory, queried_file);
    } else {
        try_write_404(stream);
    }
}

fn do_get_file(stream: TcpStream, directory: String, queried_file: &str) {
    let contents_result = fs::read_to_string(directory + queried_file);

    match contents_result {
        Ok(contents) => {
            try_write(
                stream,
                format!(
                    "HTTP/1.1 200 OK\r\n\
                            Content-Type: application/octet-stream\r\n\
                            Content-Length: {}\r\n\r\n\
                            {contents}\r\n\r\n",
                    contents.len()
                )
                .as_str(),
            );
        }
        _ => {
            try_write_404(stream);
        }
    }
}

fn do_write_file(stream: TcpStream, req: HttpRequest, directory: String, queried_file: &str) {
    fs::write(directory + queried_file, req.body).expect("Unable to write file");
    try_write(stream, "HTTP/1.1 201 Created\r\n\r\n");
}

fn trim_newlines(str: &str) -> String {
    let mut string = String::from(str);
    let truncate_len = string.trim_end_matches(&['\r', '\n'][..]).len();
    string.truncate(truncate_len);

    string
}

fn try_write(mut stream: TcpStream, buf: &str) {
    if let Err(err) = stream.write(buf.as_bytes()) {
        println!("Could not write buffer to stream. Error: {err}");

        let res_write_500 = stream.write("HTTP/1.1 500 Internal Server Error\r\n\r\n".as_bytes());

        if let Err(err) = res_write_500 {
            println!("Could not write 500 to stream. Error: {err}");
        };
    };
}

fn try_write_404(stream: TcpStream) {
    try_write(stream, "HTTP/1.1 404 Not Found\r\n\r\n");
}
