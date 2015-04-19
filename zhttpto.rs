//
// zhttpto.rs
//
// Starting code for PA1
// Running on Rust 1.0.0-nightly build 2015-02-21
//
// Note that this code has serious security risks! You should not run it
// on any system with access to sensitive files.
//
// Brandeis University - cs146a - Spring 2015


use std::old_io::{Acceptor, Listener, TcpListener};
use std::str;
use std::thread::Thread;
use std::os;
use std::old_io::BufferedReader;
use std::old_io::File;
use std::old_io::fs::PathExtensions;

fn main() {
    let addr = "127.0.0.1:4414";
    
    let mut acceptor = TcpListener::bind(addr).unwrap().listen().unwrap();
    static mut visitor_count: u32 = 0;
    
    println!("Listening on [{}] ...", addr);
    
    for stream in acceptor.incoming() {
        match stream {
            Err(_) => (),
            Ok(mut stream) => {
		
                // Spawn a thread to handle the connection
                Thread::spawn(move|| {
                    match stream.peer_name() {
                        Err(_) => (),
                        Ok(pn) => println!("Received connection from: [{}]", pn),
                    }

                    let mut buf = [0 ;500];
                    let _ = stream.read(&mut buf);
		    let mut request = "";
                    match str::from_utf8(&buf) {
                        Err(error) => println!("Received request error:\n{}", error),
                        Ok(body) => request = body,

                    }

		    let mut info = request.split_str(" ");
		    let mut info_array = info.collect::<Vec<&str>>();
                    //let path = format!("{}{}", ".",info_array[1]);
		   
	            //println!("{}",path);
		    //println!("{}",info_array[1]);
		    
	
		    unsafe{visitor_count += 1};

		    if (info_array[1] == "/") {
                    unsafe{let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n
                         <doctype !html><html><head><title>Hello, Rust!</title>
                         <style>body {{ background-color: #111; color: #FFEEAA }}
                                h1 {{ font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm red}}
                                h2 {{ font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm blue}}
                         </style></head>
                         <body>
                         <h1>Greetings, Krusty!</h1>

			 <h2>Number of visitors = {}</h2>
			
                         </body></html>\r\n", visitor_count);
                    let _ = stream.write(response.as_bytes());}}

		    else {
			
			//create the path with . as the cwd and info_array[1] as the file location
			let path = Path::new(format!("{}{}", ".",info_array[1]));
			let mut segments = info_array[1].split_str(".");
		    	let mut segments_array = segments.collect::<Vec<&str>>();
			
			//checking if file exists
			if (path.exists()){
				
				//checking if extension is HTML
				if(segments_array[segments_array.len()-1] == "html"){

					//open file and display on webpage corresponding to path if exists
					let mut file = BufferedReader::new(File::open(&path));
					let _ = stream.write("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n".as_bytes());
					for line in file.lines() {
						let response = line.unwrap();
						let _ = stream.write(response.as_bytes());
					}
				//Non-HTML Error Condition	
				} else {

				

				let response = format!(
                        		"HTTP/1.1 403 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n
                        	 	<doctype !html><html><head><title>Hello, Rust!</title>
                        	 	<style>body {{ background-color: #111; color: #FFEEAA }}
                                		h1 {{ font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm red}}
                                		h2 {{ font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm blue}}
                        	 	</style></head>
                         		<body>
                         		<h1>This server only works with HTML files</h1>

			 
			
                        		</body></html>\r\n");

                   			let _ = stream.write(response.as_bytes());


				}
			//ERROR Condition for File Not Found
			} else {
				
				let response = format!(
                        		"HTTP/1.1 404 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n
                        	 	<doctype !html><html><head><title>Hello, Rust!</title>
                        	 	<style>body {{ background-color: #111; color: #FFEEAA }}
                                		h1 {{ font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm red}}
                                		h2 {{ font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm blue}}
                        	 	</style></head>
                         		<body>
                         		<h1>File Not Found</h1>

			 
			
                        		</body></html>\r\n");

                   			let _ = stream.write(response.as_bytes());
				

			
			}
		    
                    println!("Connection terminates.");
		   }
                });
            },
        }
    }

    drop(acceptor);
}
