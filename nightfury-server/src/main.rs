#![feature(if_let_guard)]
use lib::protocol::WriteNullDelimitedExt;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::exit;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;

use bufstream::BufStream;
use lib::protocol::{Request, Response};
use lib::{FSMCursor, FSMNode};

fn handle_request(req: Request, cursor: &mut FSMCursor, stream: &mut BufStream<UnixStream>) {
    match req {
        Request::Reset => {
            cursor.reset();
        }
        Request::Advance(c) => match cursor.advance(c) {
            Some(s) => {
                stream.write_with_null(
                    serde_json::to_string(&Response::Expanded(s))
                        .unwrap()
                        .as_bytes(),
                );
            }
            None => {
                stream.write_with_null(serde_json::to_string(&Response::Ok).unwrap().as_bytes());
            }
        },
        Request::AdvanceStr(str) => {
            str.chars().for_each(|c| {
                cursor.advance(c);
            });
        }
        _ => unreachable!(),
    }
}

const DEFAULT_SOCK_ADDR: &str = "./nightfury.sock";

fn main() -> std::io::Result<()> {
    ctrlc::set_handler(|| {
        std::fs::remove_file(DEFAULT_SOCK_ADDR).unwrap();
        exit(1);
    })
    .unwrap();
    let listener = UnixListener::bind("./nightfury.sock")?;
    let mut handles = Vec::new();
    let fsms = Arc::new(RwLock::new(HashMap::new()));
    fsms.write()
        .unwrap()
        .insert("test", FSMNode::new_null(None));

    // accept connections and process them, spawning a new thread for each one
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                /* connection succeeded */
                let mut stream = BufStream::new(stream);
                println!("Connection successful!");
                let fsms_clone = Arc::clone(&fsms);
                handles.push(thread::spawn(move || {
                    println!("thread init");
                    let mut buf = Vec::new(); // bad
                    let mut cursor = None;
                    while stream.read_until(0, &mut buf).expect("buf read") != 0 {
                        let str = str::from_utf8(&buf[..&buf.len() - 1]).unwrap();
                        println!("{str}");
                        let req: Request = serde_json::from_str(str).expect("deserde");
                        println!("{req:?}");
                        match req {
                            Request::Init(ref name)
                                if let Some(fsm) =
                                    fsms_clone.read().unwrap().get(&name.as_str()) =>
                            {
                                cursor = Some(FSMCursor::new(fsm))
                            }
                            Request::GetCapabilities => {
                                stream
                                    .write_with_null(
                                        serde_json::to_string(&Response::Capabilities(
                                            fsms_clone
                                                .read()
                                                .unwrap()
                                                .keys()
                                                .map(|s| String::from(*s))
                                                .collect(),
                                        ))
                                        .unwrap()
                                        .as_bytes(),
                                    )
                                    .unwrap();
                            }
                            _ => handle_request(
                                req,
                                &mut cursor.as_mut().expect("didn't call init!"),
                                &mut stream,
                            ),
                        }
                        stream.flush().expect("stream flush");
                        buf.clear();
                    }
                }));
            }
            Err(err) => {
                /* connection failed */
                eprintln!("{err}");
                break;
            }
        }
    }
    handles.into_iter().for_each(|h| {
        h.join().unwrap();
    });
    Ok(())
}
