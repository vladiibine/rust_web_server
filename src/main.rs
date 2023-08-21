/*
* [v] get rid of the warnings
* make the server actually respond to the request, not just hardcode stuff in it
* make the server handle different paths
* html templates
* have default 404 page
* have default 500 page
* fix the "connection reset by peer" bug
* the ThreadPool.execute is badly designed. We should execute whenever a new request comes in, NOT start all the threads from
   polling. That, or name the ThreadPool properly, according to what it does ("WorkerStarter")
 */
use std::ops::Add;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Lines, Read, Write};
use std::iter::TakeWhile;
use std::net::{TcpListener, TcpStream};
use std::sync;
use std::sync::Arc;
use std::thread;

type ThreadSafeTcpStreamQueue = sync::Arc<sync::Mutex<VecDeque<TcpStream>>>;


struct ThreadPool {
    num_threads: u32,
}

fn map_to_string(map: HashMap<String, String>, delimiter: &str) -> String {
    let mut result = String::new();
    for (key, value) in map {
        result = result.add(format!("{}{}{}\r\n", key, delimiter, value).as_str());
    }
    result
}

struct HttpResponse {
    status_code: Option<u32>,
    status_message: Option<String>,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    raw_query: String,
    query: HashMap<String, Vec<String>>,
    // todo
    body: String,
    headers: HashMap<String, String>,
    http_version: String,
}

fn get_response(request: HttpRequest) -> HttpResponse {
    let mut headers = HashMap::new();
    headers.insert(String::from("Some-Header"), String::from("ASdf"));
    let body = format!("Let this be the response body!\n The request was: {:?}", request);
    HttpResponse {
        status_code: Some(200),
        status_message: Some(String::from("ok")),
        headers: Some(headers),
        body: Some(String::from(body)),
    }
}

fn process_request(request_queue: ThreadSafeTcpStreamQueue) -> ThreadSafeTcpStreamQueue {
    // [v] get the status and message
    // [v] get the headers
    // [v] get the body
    // [v] move the headers and body to a new function
    // []use some regex-from-path to handling-function engine, to delegate dynamically

    match request_queue.lock() {
        Ok(mut guarded_stream) => {
            match guarded_stream.pop_front() {
                None => {
                    // panic!("Could not get a tcp stream from the MutexGuard/dequeue for some reason")
                }
                Some(mut stream) => {
                    let http_request_res = parse_request_stream(&mut stream);
                    match http_request_res {
                        Ok(http_request) => {
                            let http_response = get_response(http_request);

                            let default_headers = HashMap::from([(String::from("Server"), String::from("rust"))]);
                            let status_line = format!("{} {}\r\n", http_response.status_code.unwrap_or(200), http_response.status_message.unwrap_or(String::from("OK")));

                            let mut all_headers = HashMap::new();
                            all_headers.extend(default_headers);
                            all_headers.extend(http_response.headers.unwrap_or(HashMap::new()));
                            let header_string = map_to_string(all_headers, ": ");
                            let body = http_response.body.unwrap_or(String::from(""));

                            stream.write(format!("HTTP/1.1 {status_line}{header_string}\r\n{body}").as_bytes())
                                .expect("Should be able to write to the tcp stream");
                            match stream.flush() {
                                Ok(()) => { /*we don't do anything here, this means success*/println!("Successfully sent back a response") }
                                Err(err) => { println!("Could not flush stream {:?}. Original error: {}", stream, err); }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        Err(err) => {
            panic!("Could not get a MutexGuard, with err {}", err)
        }
    };
    request_queue
}

fn parse_request_stream(stream: &mut TcpStream) -> Result<HttpRequest, &str> {
    let mut reader = BufReader::new(stream);
    let mut my_buffer = String::new();
    let mut current_idx = 0;
    let mut content_length: i32 = 0;
    // read the first request line (method, path, http version)
    let mut string_first_line = String::new();
    let request_line = reader.read_line(&mut string_first_line);
    let mut split = string_first_line.split(" ");
    let method = split.next();
    let path_and_query = split.next();
    let http_version = split.next();
    let mut headers: HashMap<String, String> = HashMap::new();


    // read the headers, especially the content length
    loop {
        // println!("trying to read a line");
        let current_bytes_read = reader.read_line(&mut my_buffer);

        match current_bytes_read {
            Ok(bytes_read) => {
                // println!("Got {:?} bytes, and the buffer is: {:?}", &current_bytes_read, my_buffer);
                // println!("And this is the buffer atm.:{}", )
                let current_string = &my_buffer[current_idx..current_idx + &bytes_read];
                // println!("And the string that we got on the current line is {:?}", current_string);
                current_idx += bytes_read;

                if let Some((header_key, header_value)) = current_string.split_once(':') {
                    headers.insert(header_key.to_string(), header_value.trim().to_string());
                }

                if current_string.to_lowercase().starts_with("content-length"){
                    let res = current_string.trim().split_whitespace().collect::<Vec<_>>();

                    // println!("This is the content length, as string: {}", res[1]);
                    content_length = res[1].parse::<i32>().unwrap();
                }
                if current_string == "\r\n" {
                    break
                }
            }
            Err(error) => {
                println!("We got the error {:?}", error);
            }
        }
    }

    let mut bytes_vec: Vec<u8> = Vec::new();
    let mut bytes_read = reader.take(content_length as u64);
    let _ = bytes_read.read_to_end(&mut bytes_vec);
    let body_result = String::from_utf8(bytes_vec);
    let body = body_result.unwrap_or(String::new());
    let (path, raw_query) = parse_path_and_query(path_and_query.unwrap_or_default());

    Ok(HttpRequest{
        method: method.unwrap_or_default().to_string(),
        path: path.unwrap_or_default().to_string(),
        raw_query: raw_query.unwrap_or_default().to_string(),
        query: Default::default(), // todo
        body: body,
        headers,
        http_version: http_version.unwrap_or_default().trim().to_string()
    })
}

fn parse_headers(vec: Vec<std::io::Result<String>>) -> HashMap<String, String> {
    todo!();
}

fn parse_body(vec: Vec<std::io::Result<String>>) -> Vec<u8> {
    todo!();
}

fn parse_path_and_query(path_and_query: &str) -> (Option<&str>, Option<&str>) {
    let path_and_query_parts: Vec<&str> = path_and_query.split('?').collect();
    let (path, raw_query) = if path_and_query_parts.len() >= 2 {
        (Some(path_and_query_parts[0]), Some(path_and_query_parts[1]))
    } else if path_and_query_parts.len() >= 1 {
        (Some(path_and_query_parts[0]), None)
    } else {
        (None, None)
    };
    (path, raw_query)
}


impl ThreadPool {
    fn new(num_threads: u32) -> ThreadPool {
        ThreadPool { num_threads }
    }

    fn execute(&self, request_queue: ThreadSafeTcpStreamQueue) {
        for _ in 0..self.num_threads {
            let mut request_queue_clone = sync::Arc::clone(&request_queue);
            thread::spawn(move || loop {
                request_queue_clone = process_request(request_queue_clone)
                // process_request(sync::Arc::clone(&request_queue))
            });
        }
    }
}


fn main() {
    // we implement the http server/framework thing here
    let bind_address = "0.0.0.0:7878";
    let tcp_listener = TcpListener::bind(bind_address);

    // here we'll push connections or streams or smth
    let request_queue = VecDeque::new();
    let request_queue_arc = sync::Arc::new(sync::Mutex::new(request_queue));

    let thread_pool = ThreadPool::new(10);
    thread_pool.execute(Arc::clone(&request_queue_arc));


    match tcp_listener {
        Ok(listener) => {
            for incoming in listener.incoming() {
                match incoming {
                    Ok(stream) => {
                        println!("yoyo got a connection from stream {:?}", stream);
                        request_queue_arc.lock().expect("Should be able to lock the mutex").push_back(stream);
                    }
                    Err(err) => {
                        panic!("Got his error from the incoming connection, {}", err);
                    }
                }
            }
        }
        Err(err) => { panic!("Could not bind to address {}. The error was {}", bind_address, err) }
    }
}