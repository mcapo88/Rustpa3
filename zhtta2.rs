//Andy Alexander, Mark Capobianco
// zhtta.rs
//
// Starting code for PA3
// Revised to run on Rust 1.0.0 nightly - built 02-21
//
// Note that this code has serious security risks!  You should not run it 
// on any system with access to sensitive files.
// 
// Brandeis University - cs146a Spring 2015
// Dimokritos Stamatakis and Brionne Godby
// Version 1.0

// To see debug! outputs set the RUST_LOG environment variable, e.g.: export RUST_LOG="zhtta=debug"

#![feature(rustc_private)]
#![feature(libc)]
#![feature(io)]
#![feature(old_io)]
#![feature(old_path)]
#![feature(os)]
#![feature(core)]
#![feature(collections)]
#![feature(std_misc)]
#![allow(non_camel_case_types)]
#![allow(unused_must_use)]
#![allow(deprecated)]
#[macro_use]
extern crate log;
extern crate libc;
mod gash;  //this is for using gash.rs

use std::io::*;
use std::old_io::File;
use std::{os, str};
use std::old_path::posix::Path;
use std::collections::hash_map::HashMap;
use std::borrow::ToOwned;
use std::thread::Thread;
use std::old_io::fs::PathExtensions;
use std::old_io::{Acceptor, Listener};
use std::old_io::BufferedReader;
extern crate getopts;
use getopts::{optopt, getopts};

use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::channel;
use std::collections::BinaryHeap; //will be used to make priority queue
use std::cmp::Ordering;
use std::old_io::Timer;  //timer will be used to make busy-waiting more efficient (less frequent checks of count of threads running)
use std::time::Duration;

const thread_limit: usize = 30; //alter the num of threads that can be made for tasks easily here


static SERVER_NAME : &'static str = "Zhtta Version 1.0";

static IP : &'static str = "127.0.0.1";
static PORT : usize = 4414;
static WWW_DIR : &'static str = "./www";

static HTTP_OK : &'static str = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n";
static HTTP_BAD : &'static str = "HTTP/1.1 404 Not Found\r\n\r\n";

static COUNTER_STYLE : &'static str = "<doctype !html><html><head><title>Hello, Rust!</title>
             <style>body { background-color: #884414; color: #FFEEAA}
                    h1 { font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm red }
                    h2 { font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm green }
             </style></head>
             <body>";


struct HTTP_Request {
    // Use peer_name as the key to access TcpStream in hashmap. 

    // (Due to a bug in extra::arc in Rust 0.9, it is very inconvenient to use TcpStream without the "Freeze" bound.
    //  See issue: https://github.com/mozilla/rust/issues/12139)
    peer_name: String,
    path: Path,
    size: u64,
}
//need to make it a min heap
impl PartialEq for HTTP_Request {
    fn eq(&self, other: &Self) -> bool{
        other.size == self.size
    }
}
impl Eq for HTTP_Request {}
impl PartialOrd for HTTP_Request {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.size.partial_cmp(&self.size)
    }
}
impl Ord for HTTP_Request {
    fn cmp(&self, other: &Self) -> Ordering {
        other.size.cmp(&self.size)
    }
}

struct WebServer {
    ip: String,
    port: usize,
    www_dir_path: Path,
    
    request_queue_arc: Arc<Mutex<BinaryHeap<HTTP_Request>>>,
    stream_map_arc: Arc<Mutex<HashMap<String, std::old_io::net::tcp::TcpStream>>>,
    //make an arc containing a Map to store files- this acts as a cache
    cached_files_arc: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    being_read_arc: Arc<Mutex<HashMap<String, String>>>, //for files in process of being read
    notify_rx: Receiver<()>,
    notify_tx: Sender<()>,
}

impl WebServer {
    fn new(ip: String, port: usize, www_dir: String) -> WebServer {
        let (notify_tx, notify_rx) = channel();
        let www_dir_path = Path::new(www_dir);
        os::change_dir(&www_dir_path);

        WebServer {
            ip:ip,
            port: port,
            www_dir_path: www_dir_path,
                        
            request_queue_arc: Arc::new(Mutex::new(BinaryHeap::new())),
            stream_map_arc: Arc::new(Mutex::new(HashMap::new())),
            cached_files_arc: Arc::new(Mutex::new(HashMap::new())),
            being_read_arc: Arc::new(Mutex::new(HashMap::new())),
            
            notify_rx: notify_rx,
            notify_tx: notify_tx,
        }
    }
    
    fn run(&mut self) {
        self.listen();
        self.dequeue_static_file_request();
    }
    
    fn listen(&mut self) {
       
    	let addr = String::from_str(format!("{}:{}", self.ip, self.port).as_slice());
        let www_dir_path_str = self.www_dir_path.clone();
        let request_queue_arc = self.request_queue_arc.clone(); //tasks to schedule- want it to be a priority queue if static file, put in request queue and handles it single-threadedly
        let notify_tx = self.notify_tx.clone();
        let stream_map_arc = self.stream_map_arc.clone();
        
        Thread::spawn(move|| {
        	let listener = std::old_io::TcpListener::bind(addr.as_slice()).unwrap();
            let mut acceptor = listener.listen().unwrap();
            println!("{} listening on {} (serving from: {}).", 
                     SERVER_NAME, addr, www_dir_path_str.as_str().unwrap());
            let mut visits = 0; //initialize visitor count as 0
            for stream_raw in acceptor.incoming() {
                visits += 1; //increment visits by 1 for every incoming request
                let count = visits; //make Arc holding current num visitors
                let (queue_tx, queue_rx) = channel();
                queue_tx.send(request_queue_arc.clone());
                
                let notify_chan = notify_tx.clone();
                let stream_map_arc = stream_map_arc.clone();
                // Spawn a task to handle the connection.
                Thread::spawn(move|| {
                	//println!("visitor count is {}", local_count);
                    let request_queue_arc = queue_rx.recv().unwrap();
                    let mut stream = match stream_raw {
                        Ok(s) => {s}
				        Err(e) => { panic!("Error getting the listener stream! {}", e) }
				    };
                    let peer_name = WebServer::get_peer_name(&mut stream);
                    debug!("Got connection from {}", peer_name);
                    let mut buf: [u8;500] = [0;500];
                    stream.read(&mut buf);
                    let request_str = match str::from_utf8(&buf){
                        Ok(s) => s,
                        Err(e)=> panic!("Error reading from the listener stream! {}", e),
                    };
                    debug!("Request:\n{}", request_str);
                    let req_group: Vec<&str> = request_str.splitn(3, ' ').collect();
                    if req_group.len() > 2 {
                        let path_str = ".".to_string() + req_group[1];
                        let mut path_obj = os::getcwd().unwrap();
                        path_obj.push(path_str.clone());
                        let ext_str = match path_obj.extension_str() {
                            Some(e) => e,
                            None => "",
                        };
                       
                        debug!("Requested path: [{}]", path_obj.as_str().expect("error"));
                        debug!("Requested path: [{}]", path_str);
                        //maybe spawn new threads here to deal with each request
                             
                        if path_str.as_slice().eq("./")  {
                            debug!("===== Counter Page request =====");
                            //send count of visitors to method with counter page
                            WebServer::respond_with_counter_page(stream, count);
                            debug!("=====Terminated connection from [{}].=====", peer_name);
                        }  else if !path_obj.exists() || path_obj.is_dir() {
                            debug!("===== Error page request =====");
                            WebServer::respond_with_error_page(stream, &path_obj);
                            debug!("=====Terminated connection from [{}].=====", peer_name);
                        } else if ext_str == "shtml" { // Dynamic web pages.
                            debug!("===== Dynamic Page request =====");
                            WebServer::respond_with_dynamic_page(stream, &path_obj);
                            debug!("=====Terminated connection from [{}].=====", peer_name);
                        } else { 
                            debug!("===== Static Page request =====");
                            WebServer::enqueue_static_file_request(stream, &path_obj, stream_map_arc, request_queue_arc, notify_chan);
                        }
                    }
                });
            }
		});
    }

    fn respond_with_error_page(stream: std::old_io::net::tcp::TcpStream, path: &Path) {
		let mut stream = stream;
		let msg: String= format!("Cannot open: {}", path.as_str().expect("invalid path"));
		stream.write(HTTP_BAD.as_bytes());
		stream.write(msg.as_bytes());
    }

    // Safe visitor counter.
    fn respond_with_counter_page(stream: std::old_io::net::tcp::TcpStream, local_count: usize) {
        let mut stream = stream;
        let response: String = 
            format!("{}{}<h1>Greetings, Krusty!</h1><h2>Visitor count: {}</h2></body></html>\r\n", 
                    HTTP_OK, COUNTER_STYLE, 
                    local_count);
        debug!("Responding to counter request");
        stream.write(response.as_bytes());
    }
    
    // TODO: Streaming file.
    // TODO: Application-layer file caching.
    fn respond_with_static_file(stream: std::old_io::net::tcp::TcpStream, path: &Path, cache: Arc<Mutex<HashMap<String, Vec<u8>>>>, being_read: Arc<Mutex<HashMap<String, String>>>) {
        let mut timer = Timer::new().unwrap();
        //boolean saying whether file in cache or not
        let mut was_cached = 0;
        let mut file_reader: File;
        //first, get the map that stores all the files in cache, acquire lock, and set boolean indicating whether or not it was in cache
        {
            let mut cache_map = cache.lock().unwrap();
            //path as string is key for cached file
            let fname = path.as_str().unwrap();
            //if file is in cache, set was_cached to "true" (1)
            if cache_map.contains_key(fname) {
                println!("in cache");
                was_cached = 1;
            } 
        }
        let fname = path.as_str().unwrap();
        let f_string = fname.to_string();
        let read_val = "reading".to_string();
        let mut being_read_bool = 0;
        //if file was not in the cache, but is in process of being read
        //by another thread, other threads should busy wait for file to be
        //put in cache
        {
            let mut reading = being_read.lock().unwrap();
            if reading.contains_key(fname) {
                println!("file currently being read from disk");
                being_read_bool = 1;
            }
        }
        //if file was not in cache, but is currently being read from disk
        //other threads trying to get the file should spin until in cache
        if was_cached == 0 && being_read_bool == 1
        {
            while being_read_bool == 1 {
                {
                    //within busy wait, check being_read periodically
                    let mut reading = being_read.lock().unwrap();
                    if !reading.contains_key(fname)
                    {
                        //when no longer being read, set being_read to false
                        being_read_bool = 0;
                        was_cached = 1; //and, the file is now in cache
                        //println!("still being read..."); 
                    }
                }
                //sleep me so it's not constantly checking the counter
                timer.sleep(Duration::milliseconds(10)); 
            }
        }
        //if file wasn't in cache, read from disk
        if was_cached == 0 { 
            //was not cached, but about to be read from disk- put in being_read
            {
                let mut reading = being_read.lock().unwrap();
                let f_string2 = fname.to_string();
                reading.insert(f_string2, read_val);
            }
            //{
            //let mut cache_map = cache.lock().unwrap();
            //read from disk
            println!("NOT in cache");
            file_reader = File::open(path).unwrap();
            let file_stats = file_reader.stat();
            let file_size = file_stats.unwrap().size;
            let mut reader = BufferedReader::new(file_reader);
            let mut stream = stream;
            stream.write(HTTP_OK.as_bytes());
            //now stream file and also append it into contents- this can
            //take a while especially for bigger files, but this does not
            //happen within a lock, so other threads can run concurrently
            //with this reading
            let mut contents: Vec<u8> = Vec::with_capacity(file_size as usize);
            for line in reader.lines().filter_map(|result| result.ok()) {
                let l2 = line.clone();
                let mut line_vec: Vec<u8> = line.into_bytes();
                contents.append(&mut line_vec);
                let _ = stream.write_all(l2.as_bytes());
            }
            //after read from disk, acquire lock and put file in cache
            {
                let mut cache_map = cache.lock().unwrap();
                cache_map.insert(f_string, contents);
                was_cached = 1;
                //this file is no longer in the process of being read
                let mut reading = being_read.lock().unwrap();
                reading.remove(fname);
                being_read_bool = 1;
            }   
        }
        //file was in cache- read it out of the cache
        else{ 
            let fname = path.as_str().unwrap();
            println!("read from cache");
            //acquire lock to read file out of cache
            {
                let mut cache_map = cache.lock().unwrap();
                let mut file_reader_option = cache_map.get(fname);
                let contents = file_reader_option.unwrap();
                let mut stream = stream;
                stream.write(HTTP_OK.as_bytes());
                stream.write(contents.as_slice());
            }
        }
    }
    
    //Server-side gashing.
    fn respond_with_dynamic_page(stream: std::old_io::net::tcp::TcpStream, path: &Path) {
      // for now, just serve as static file
      //WebServer::respond_with_static_file(stream, path);
      let mut stream = stream;
      let response = HTTP_OK;
      //let mut resp: &str = "";
      //let resp_str: String = ("").to_string();
      let mut file = BufferedReader::new(File::open(path));
      stream.write(response.as_bytes());  //start the stream with HTTP_OK
      for line in file.lines(){
        let l = line.unwrap();
        let l_str: String = l.to_string();
        let l_split = l_str.split_str("<!--#exec");
        let l_vec = l_split.collect::<Vec<&str>>();
        if l_vec.len() > 1 {  //if there was an embedded shell command
            //println!("shell command");
            stream.write(l_vec[0].as_bytes());
            let middle = l_vec[1];
            let mid_split = middle.split_str("-->");
            let mid_vec = mid_split.collect::<Vec<&str>>();
            let in_quotes = mid_vec[0];
            let openq_split = in_quotes.split_str("\"");
            let openq_vec = openq_split.collect::<Vec<&str>>();
            //println!("{}", openq_vec[1]);
            //cmd for gash is in openq_vec[1]
            let command = openq_vec[1];
            //code below prints out results of Shell's running of embedded command
            let cmd_result: Receiver<gash::Packet> = gash::Shell::new("").run_cmdline(command);
            loop{
				let test: gash::Packet = match cmd_result.recv(){
					Err(why) => {break;},
					Ok(res) => {res},
		        };
		        stream.write(test.to_string().as_bytes());
		        if test.end_of_stream {
			        break;
		        }
		    }
		    if l_vec.len() > 2 { //if there was more AFTER the embedded shell command
		        stream.write(l_vec[2].as_bytes());
		    }  
        } else {  //if there was no embedded shell command, just write line
            stream.write(l.as_bytes());
        }
      }
    }
    
    // TODO: Smarter Scheduling.
    fn enqueue_static_file_request(stream: std::old_io::net::tcp::TcpStream, path_obj: &Path, stream_map_arc: Arc<Mutex<HashMap<String, std::old_io::net::tcp::TcpStream>>>, req_queue_arc: Arc<Mutex<BinaryHeap<HTTP_Request>>>, notify_chan: Sender<()>) {
    	// Save stream in hashmap for later response.
        let mut stream = stream;
        let peer_name = WebServer::get_peer_name(&mut stream);
        let (stream_tx, stream_rx) = channel();
        stream_tx.send(stream);
        let stream = match stream_rx.recv(){
            Ok(s) => s,
            Err(e) => panic!("There was an error while receiving from the stream channel! {}", e),
        };
        let local_stream_map = stream_map_arc.clone();
        {   // make sure we request the lock inside a block with different scope, so that we give it back at the end of that block
            let mut local_stream_map = local_stream_map.lock().unwrap();
            local_stream_map.insert(peer_name.clone(), stream);
        }

        // Enqueue the HTTP request.
        // TOCHECK: it was ~path_obj.clone(), make sure in which order are ~ and clone() executed
        let size = path_obj.stat().unwrap().size;
        let req = HTTP_Request { peer_name: peer_name.clone(), path: path_obj.clone(), size: size };
        let (req_tx, req_rx) = channel();
        req_tx.send(req);
        debug!("Waiting for queue mutex lock.");
        
        let local_req_queue = req_queue_arc.clone();
        {   // make sure we request the lock inside a block with different scope, so that we give it back at the end of that block
            let mut local_req_queue = local_req_queue.lock().unwrap();
            let req: HTTP_Request = match req_rx.recv(){
                Ok(s) => s,
                Err(e) => panic!("There was an error while receiving from the request channel! {}", e),
            };
            local_req_queue.push(req);
            debug!("A new request enqueued, now the length of queue is {}.", local_req_queue.len());
            notify_chan.send(()); // Send incoming notification to responder task. 
        }
    }
    
    // TODO: Smarter Scheduling.
    fn dequeue_static_file_request(&mut self) {
        let req_queue_get = self.request_queue_arc.clone();
        let stream_map_get = self.stream_map_arc.clone();
        // Receiver<> cannot be sent to another task. So we have to make this task as the main task that can access self.notify_rx.
        let (request_tx, request_rx) = channel();
        let mut thread_count = Arc::new(Mutex::new(0)); //count how many threads are running for tasks
        let mut counter = 0;
        let mut timer = Timer::new().unwrap();
        loop 
        { 
            
            self.notify_rx.recv();    // waiting for new request enqueued.
            {   // make sure we request the lock inside a block with different scope, so that we give it back at the end of that block
                let mut req_queue = req_queue_get.lock().unwrap();
                if req_queue.len() > 0 {
                    let req = req_queue.pop();
                    debug!("A new request dequeued, now the length of queue is {}.", req_queue.len());
                    request_tx.send(req);
                }
            }

            let request = match request_rx.recv(){
                Ok(s) => s.unwrap(),
                Err(e) => panic!("There was an error while receiving from the request channel! {}", e),
            };
            // Get stream from hashmap.
            let (stream_tx, stream_rx) = channel();
            {   // make sure we request the lock inside a block with different scope, so that we give it back at the end of that block
                let mut stream_map = stream_map_get.lock().unwrap();
                let r2 = &request.peer_name;
                //let r3 = &request.path;
                //println!("next req is {:?}", r3);
                let stream = stream_map.remove(r2).expect("no option tcpstream");
                stream_tx.send(stream);
            }
            // TODO: Spawning more tasks to respond the dequeued requests concurrently. You may need a semophore to control the concurrency.
            
            //let tcount = thread_count.clone();
            //let mut count = tcount.lock().unwrap();
            //println!("times thru loop: {}", loopct);
            //println!("counter is {}", counter);
            
            //check the count of currently running threads
            {
            counter = *(thread_count.lock().unwrap());
                println!("init {} - {}", counter, thread_limit);
            }
            //if count is greater than or equal to limit, busy wait
            
            while counter >= thread_limit {
                {
                //within busy wait, check the counter periodically
                counter = *(thread_count.lock().unwrap());
                 //println!("waiting {} - {}", counter, thread_limit);
                }
                //sleep me so it's not constantly checking the counter
                timer.sleep(Duration::milliseconds(10)); 
            }
            let cache = self.cached_files_arc.clone();
            let being_read = self.being_read_arc.clone();
            {
                {
                    let mut c =thread_count.lock().unwrap();  //lock the count so it can be incremented
                    *c += 1;
                    println!("INC: threads running is {}", *c);
                }
                let tc = thread_count.clone();
                Thread::spawn(move || {
                let stream = match stream_rx.recv(){
                    Ok(s) => s,
                    Err(e) => panic!("There was an error while receiving from the stream channel! {}", e),
                };
                //println!("threads running is {}", thread_count);
                let r = &request.path;
                WebServer::respond_with_static_file(stream, r, cache, being_read);
                // Close stream automatically.
                debug!("=====Terminated connection from [{:?}].=====", r);
                {
                    let mut d = tc.lock().unwrap(); //lock the count so it can be decremented
                    *d -= 1;
                    println!("DEC: threads running is {}", *d);
                }
                
                });
            }

           
        }
    }
    
    fn get_peer_name(stream: &mut std::old_io::net::tcp::TcpStream) -> String{
        match stream.peer_name(){
            Ok(s) => {format!("{}:{}", s.ip, s.port)}
            Err(e) => {panic!("Error while getting the stream name! {}", e)}
        }
    }
}

fn get_args() -> (String, usize, String) {
	fn print_usage(program: &str) {
        println!("Usage: {} [options]", program);
        println!("--ip     \tIP address, \"{}\" by default.", IP);
        println!("--port   \tport number, \"{}\" by default.", PORT);
        println!("--www    \tworking directory, \"{}\" by default", WWW_DIR);
        println!("-h --help \tUsage");
    }
    
    /* Begin processing program arguments and initiate the parameters. */
    let args = os::args();
    let program = args[0].clone();
    
    let opts = [
        getopts::optopt("", "ip", "The IP address to bind to", "IP"),
        getopts::optopt("", "port", "The Port to bind to", "PORT"),
        getopts::optopt("", "www", "The www directory", "WWW_DIR"),
        getopts::optflag("h", "help", "Display help"),
    ];

    let matches = match getopts::getopts(args.tail(), &opts) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_err_msg()) }
    };

    if matches.opt_present("h") || matches.opt_present("help") {
        print_usage(program.as_slice());
        unsafe { libc::exit(1); }
    }
    
    let ip_str = if matches.opt_present("ip") {
                    matches.opt_str("ip").expect("invalid ip address?").to_owned()
                 } else {
                    IP.to_owned()
                 };
    
    let port:usize = if matches.opt_present("port") {
        let input_port = matches.opt_str("port").expect("Invalid port number?").trim().parse::<usize>().ok();
        match input_port {
            Some(port) => port,
            None => panic!("Invalid port number?"),
        }
    } else {
        PORT
    };
    
    let www_dir_str = if matches.opt_present("www") {
                        matches.opt_str("www").expect("invalid www argument?") 
                      } else { WWW_DIR.to_owned() };
    
    (ip_str, port, www_dir_str)    
}

fn main() {
    let (ip_str, port, www_dir_str) = get_args();
    let mut zhtta = WebServer::new(ip_str, port, www_dir_str);
    zhtta.run();
}
