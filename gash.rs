//
// gash.rs
//
// The Brionne Godby Version
// 

extern crate getopts;
extern crate collections;
use getopts::{optopt, getopts};
use std::old_io::{BufferedReader, BufferedWriter, stdin, File};
use std::process::{Command, Stdio};

use std::{old_io, os};
use std::sync::mpsc::{channel, Sender, Receiver};
use self::collections::borrow::{Cow, ToOwned};
use std::slice::SliceExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::old_path::BytesContainer;
use std::thread::Thread;
use std::str;
use std::slice::bytes;
use std::io::prelude::*;

const BUFF_SIZE: usize =64;
const ZERO_BUFF: [u8; BUFF_SIZE]=[0;BUFF_SIZE];

const START_PACKET: Packet = Packet{payload:ZERO_BUFF, length: 0, end_of_stream: false};
const END_PACKET: Packet = Packet{payload:ZERO_BUFF, length: 0, end_of_stream: true};
struct Packet {
	payload: [u8; BUFF_SIZE],
	length: usize,
	end_of_stream: bool
}

struct Command_Parse {
	cmd_line: String,
	input_file: String,
	output_file: String,
	background_flag: bool,
}

impl Command_Parse {
	fn new(line: &str) -> Command_Parse{
			let bg = line.ends_with("&");
			let mut length = line.len();
			if bg {
				length -=1;
			}
			let mut redirect_out = length;
			let mut redirect_in = length;
			if line.contains(">") {
				redirect_out = line.rfind('>').unwrap();
			}
			if line.contains("<") {
				redirect_in = line.rfind('<').unwrap();
			}

			if(redirect_in > redirect_out && redirect_out!= length){
					if redirect_in != length {
						return Command_Parse{	cmd_line: line[0..redirect_out].to_string(),	input_file: line[redirect_in+1..length].to_string(),	output_file: line[redirect_out+1..redirect_in].to_string(),	background_flag: bg};
		
					} else {
						return Command_Parse{	cmd_line: line[0..redirect_out].to_string(), input_file: String::new(),	output_file: line[redirect_out+1..redirect_in].to_string(),	background_flag: bg};
					}
			} else if redirect_in != length{
					if redirect_out != length {
						return Command_Parse{	cmd_line: line[0..redirect_in].to_string(),	input_file: line[redirect_in+1..redirect_out].to_string(),	output_file: line[redirect_out+1..length].to_string(),	background_flag: bg};
					}	else {
						return Command_Parse{	cmd_line: line[0..redirect_in].to_string(),	input_file: line[redirect_in+1..redirect_out].to_string(),	output_file: String::new(),	background_flag: bg};
					}			
			} else {
				return Command_Parse{	cmd_line: line[0..length].to_string(),	input_file: String::new(),	output_file: String::new(),	background_flag: bg};
			}
			
		}
}

impl Packet {
	fn to_string(&self) -> Cow<str>{
		if(self.length<BUFF_SIZE){
			return String::from_utf8_lossy(&self.payload[0..self.length]);

		}else{
			return String::from_utf8_lossy(&self.payload);
		}
	}
	fn get_bytes(&self) -> &[u8]{
		if(self.length<BUFF_SIZE){
			return &self.payload[0..self.length];
		}else{
			return &self.payload;
		}
	}
}

struct Shell<'a> {
    cmd_prompt: &'a str,
		history: Rc<RefCell<Vec<String>>>
}

impl <'a>Shell<'a> {
    fn new(prompt_str: &'a str) -> Shell<'a> {
        Shell { cmd_prompt: prompt_str, history: Rc::new(RefCell::new(Vec::new()))}
    }

    fn run(&self) {
      let mut stdin = BufferedReader::new(stdin());
      loop {
        old_io::stdio::print(self.cmd_prompt.as_slice());
        old_io::stdio::flush();
			
        let mut l = stdin.read_line().unwrap();
				let mut line = l.trim();
				match line {
          ""        =>  { continue; }
          "exit"    =>  { return; }
          _         =>  { self.run_cmdline(line); }
        }
				
      }
    }

		fn add_to_history(&self, new_line:String){
//			let mut history= *self.history;
			self.history.borrow_mut().push(new_line + "\n");
		}

		fn run_cd(&self, cmd_line: &str, s_in: Receiver<Packet>, s_out: Sender<Packet>) {
			os::change_dir(&Path::new(cmd_line.splitn(1, ' ').nth(1).unwrap()));
			s_out.send(Packet{payload:ZERO_BUFF, length:0, end_of_stream: true});
		}	

		fn run_history(&self, s_in: Receiver<Packet>, s_out: Sender<Packet>) {
			let history_c = self.history.borrow_mut().clone();
			
			let iterator = history_c.iter();
			let mut pos:usize = 0;
			let mut buf: [u8; BUFF_SIZE]=[0;BUFF_SIZE];
			for line in iterator {			
				let payl = line.as_bytes();
				for by in payl{
					if pos >= BUFF_SIZE { 
						pos=0;
						s_out.send(Packet{payload:buf, length:BUFF_SIZE, end_of_stream: false});
					}
					buf[pos]=*by;
					pos +=1;
				}		
			}
			for x in pos..BUFF_SIZE-1 {
				buf[x]=0;
			}
			s_out.send(Packet{payload:buf, length:pos, end_of_stream: true});
		}
		

		
    fn run_cmdline(&self, line: &str) {
			
			self.add_to_history(line.to_string());
			let command = Command_Parse::new(line);
			//println!("{} in bg: {}\n in: {} \n out: {}", command.cmd_line, command.background_flag, command.input_file, command.output_file);
			let mut last_out: Receiver<Packet>;
				{
					let (inc, out) = channel::<Packet>();
					if(!command.input_file.is_empty()){
						self.read_file(inc, &Path::new(command.input_file.clone().trim()));
					}else{
						inc.send(END_PACKET);
					}
					last_out = out;
				}
				let segments = command.cmd_line.split('|');
				for cmdt in segments{
					let cmd_line=cmdt.trim();
	        let program = cmd_line.splitn(1, ' ').nth(0).expect("no program");
					let (inc, out) = channel::<Packet>();
	        match program {
							"cd"      =>  { self.run_cd(cmd_line, last_out, inc); }
							"history" =>  { self.run_history(last_out, inc); }
	            _         =>  { self.spawn_command(cmd_line, last_out, inc); }
	        }
					//according to all the documents we absolutely shouldn't do the above line
					//according to practice, failure to do the above line results in the channel closing before it can be used
					//possibly due to some dereferencing issue when put into a vector
					last_out=out;
				}
				//everything runs in the background except for the printer/writer threads. So the only difference in background process
			if command.background_flag {						
					if(command.output_file.is_empty()){
						Thread::spawn(move|| {
							Shell::print_results(last_out);
						});
					} else {
						let f_name=command.output_file.clone();
						Thread::spawn(move|| {
							Shell::write_results(last_out, f_name);
						});
					}						
			}else { 
				if(command.output_file.is_empty()){
					Shell::print_results(last_out);
				} else {
					Shell::write_results(last_out, command.output_file);
				}	
			}
		}
    fn run_cmd(&self, program: &str, line: &str, s_in: Receiver<Packet>, s_out: Sender<Packet>) {
				let program_p = program.to_string().clone();
				let cmd_line=line.to_string().clone();
        if self.cmd_exists(program) {
					Thread::spawn(move|| {
						let temp_v:Vec<&str>=cmd_line.split(' ').filter_map(|x| {if x == "" {None} else {Some(x)}}).collect();
						let args:&[&str] =temp_v.tail();
						let process = match Command::new(program_p.as_slice()).args(args).stdin(Stdio::capture()).stdout(Stdio::capture()).spawn() {
							Err(why) => panic!("couldn't spawn {}: {}", program_p, why.description()),
							Ok(process) => process,
						};
						{
							let mut stin = process.stdin.unwrap();
							let program_in = program_p.clone();
							Thread::spawn(move || {
							loop {
								let the_packet: Packet = match s_in.recv(){
									Err(why) => {//println!("error reading from recv"); 
											break;},
									Ok(num) => {//println!("{}", num.container_as_str().unwrap()); 
										num},
								};
								match stin.write(the_packet.get_bytes()) {
									Err(why) => {//println!("error writing to stdin");
										break;},
									Ok(hm) => {},
								}
								if the_packet.end_of_stream {
									//println!("got end of stream in program {}", program_p);
									break;
								}
								// `stdin` gets `drop`ed here, and the pipe is closed
								// This is very important, otherwise `wc` wouldn't start processing the
								// input we just sent
							}
							});
						}

						// The `stdout` field also has type `Option<PipeStream>`
						// the `as_mut` method will return a mutable reference to the value
						// wrapped in a `Some` variant
						let mut stout = process.stdout.unwrap();
						let program_out = program_p.clone();
						//println!("preparing to read stdout in {}", program_p);
						let out_thread = Thread::spawn(move || {
							//println!("Start write output {}", program_out);
							loop{
								let mut buf: [u8; BUFF_SIZE]=[0;BUFF_SIZE];
								let buffer_s = match stout.read(&mut buf) {
									Err(why) => {//panic!("couldn't read stdout: {}",	why.description()); 
									break;},
									Ok(num) => {//println!("{} read {} bytes to buffer", program_p, num); 
										if num ==0{ break;} num},
								};			
								let result = match s_out.send(Packet{payload:buf, length:buffer_s, end_of_stream: buffer_s<BUFF_SIZE}){
									Err(why) => {//panic!("couldn't write to stream in {} \n reason: {}",program_p, why);
										break; },
									Ok(_) => {},
								};
							}
						//println!("End write output {}", program_out);
						});
						//println!("finished reading stdout in {}", program_p);
					});
        } else {
            return println!("{}: command not found", program_p);
        }
    }

    fn cmd_exists(&self, cmd_path: &str) -> bool {
        Command::new("which").arg(cmd_path).stdout(Stdio::capture()).status().unwrap().success()
    }

		fn spawn_command(&self, cmd_line: &str, s_in: Receiver<Packet>, s_out: Sender<Packet>){
				let argv: Vec<&str> = cmd_line.split(' ').filter_map(|x| {
						if x == "" {
								None
						} else {
								Some(x)
						}
				}).collect();

				match argv.first() {
						Some(&program) => self.run_cmd(program, cmd_line, s_in, s_out),
						None => (),
				}; 				
		}

		fn print_results(s_in: Receiver<Packet>){
			loop{
				let test: Packet = match s_in.recv(){
					Err(why) => {break;},
					Ok(res) => {res},
				};
				print!("{}", test.to_string());
				if test.end_of_stream {
					break;
				}
			}
		}

		fn write_results(s_in: Receiver<Packet>, filename: String){
			match File::create(&Path::new(filename.trim())) {
				Ok(file) => {
					let mut writer = BufferedWriter::new(file);
					loop{
						let msg: Packet = match s_in.recv(){
							Err(why) => {println!("error breaking"); break;},
							Ok(res) => {res},
						};
						writer.write(msg.get_bytes());
						if msg.end_of_stream {
							break;
						}
					}
					writer.flush().unwrap();
				},
				Err(_) =>{ println!("No such file or directory");}
			};	
		} 
		
		fn read_file(&self, s_out: Sender<Packet>, filename: &Path){
			let filename = filename.clone();
			Thread::spawn(move || {
				match File::open(&filename) {
					Ok(file) => {
						let mut reader = BufferedReader::new(file);
						loop{
							let mut buf: [u8; BUFF_SIZE]=[0;BUFF_SIZE];
							let buffer_s = match reader.read(&mut buf) {
								Err(why) => {
								break;},
								Ok(num) => {//println!("{} read {} bytes to buffer", program_p, num); 
									if num ==0{ break;} num},
							};			
							let result = match s_out.send(Packet{payload:buf, length:buffer_s, end_of_stream: buffer_s<BUFF_SIZE}){
								Err(why) => {
									break; },
								Ok(_) => {},
							};
						}
					},
					Err(_) =>{ println!("No such file or directory");}
				};	
			});
		} 

}

fn get_cmdline_from_args() -> Option<String> {
    /* Begin processing program arguments and initiate the parameters. */
    let args = os::args();

    let opts = &[
        getopts::optopt("c", "", "", "")
    ];

    getopts::getopts(args.tail(), opts).unwrap().opt_str("c")
}

fn main() {
    let opt_cmd_line = get_cmdline_from_args();
    match opt_cmd_line {
        Some(cmd_line) => Shell::new("").run_cmdline(cmd_line.as_slice()),
        None           => Shell::new("gash > ").run(),
    }
}
