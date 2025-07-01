#![feature(if_let_guard)]
use lib::protocol::WriteNullDelimitedExt;
use lib::{FSMNodeWrapper, ToCSV, get_test_fsm};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::Write;
use std::io::{BufRead, read_to_string};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::exit;
use std::sync::Arc;
use std::sync::RwLock;
use std::{env, thread};

use bufstream::BufStream;
use lib::protocol::{Request, Response};
use lib::{FSMCursor, FSMNode};

fn handle_request(
    req: Request,
    cursor: &mut FSMCursor,
    stream: &mut BufStream<UnixStream>,
) -> std::io::Result<()> {
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
                )?;
            }
            None => {
                stream.write_with_null(serde_json::to_string(&Response::Ok).unwrap().as_bytes())?;
            }
        },
        Request::AdvanceStr(str) => {
            str.chars().for_each(|c| {
                cursor.advance(c);
            });
        }
        _ => unreachable!(),
    }
    Ok(())
}

const DEFAULT_SOCK_ADDR: &str = ".";
fn get_sock_path() -> String {
    if let Ok(path) = env::var("XDG_RUNTIME_DIR") {
        return path;
    }
    if let Ok(euid) = env::var("EUID") {
        let default_rtd = format!("/run/user/{euid}");
        if std::path::Path::new(&default_rtd).is_dir() {
            return default_rtd;
        }
    }
    DEFAULT_SOCK_ADDR.to_string()
}

fn get_sock_addr() -> String {
    let mut dir = get_sock_path();
    dir.push_str("/nightfury.sock");
    dir
}

fn cleanup(sock_addr: &str) {
    std::fs::remove_file(&sock_addr).unwrap();
}

fn main() -> std::io::Result<()> {
    let sock_addr = get_sock_addr();
    let sock_addr_clone = sock_addr.clone();
    ctrlc::set_handler(move || {
        cleanup(&sock_addr_clone);
        exit(1);
    })
    .unwrap();
    let listener = UnixListener::bind(&sock_addr)?;
    let mut handles = Vec::new();
    let fsms = Arc::new(RwLock::new(HashMap::new()));

    fsms.write()
        .unwrap()
        .insert("c".to_string(), get_test_fsm());

    if let Ok(dir) = env::var("NIGHTFURY_FSMDIR") {
        for fsm in read_dir(dir)? {
            match fsm {
                Ok(fsm) => {
                    let mut fsms = fsms.write().unwrap();
                    let file_name = fsm.file_name();
                    // if let Some(file_name) = file_name.to_str()
                    //     && let Ok(file) = File::open(&file_name)
                    //     && let Ok(csv) = read_to_string(file)
                    // {
                    //     fsms.insert(file_name.to_string(), FSMNodeWrapper::from_csv(&csv));
                    // }
                    match file_name.to_str() {
                        Some(fsm_name) => {
                            fsms.insert(
                                fsm_name.to_string(),
                                // TODO: cleanup
                                FSMNodeWrapper::from_csv(
                                    &read_to_string(File::open(fsm.path()).unwrap()).unwrap(),
                                ),
                            );
                        }
                        None => {
                            eprintln!("Filename isn't valid Unicode!");
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error reading fsm dir entry:");
                    eprint!("{err}");
                }
            }
        }
    }

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
                        println!("str: {str}");
                        let req: Request = serde_json::from_str(str).expect("deserde");
                        println!("req: {req:?}");
                        match req {
                            Request::Init(ref name)
                                if let Some(fsm) =
                                    fsms_clone.read().unwrap().get(&*name.as_str()) =>
                            {
                                cursor = Some(FSMCursor::new(fsm))
                            }
                            Request::GetCapabilities => stream.write_with_null(
                                serde_json::to_string(&Response::Capabilities(
                                    fsms_clone
                                        .read()
                                        .unwrap()
                                        .keys()
                                        .map(|s| String::from(s))
                                        .collect(),
                                ))
                                .unwrap()
                                .as_bytes(),
                            )?,
                            _ => handle_request(
                                req,
                                &mut cursor.as_mut().expect("didn't call init!"),
                                &mut stream,
                            )?,
                        }
                        stream.flush().expect("stream flush");
                        buf.clear();
                    }
                    std::io::Result::<()>::Ok(())
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
        if let Err(err) = h.join().unwrap() {
            eprintln!("join: {err}");
        }
    });
    cleanup(&sock_addr);
    Ok(())
}
