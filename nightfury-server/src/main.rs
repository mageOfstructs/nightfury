#![feature(if_let_guard)]
use lib::protocol::{ReadRequest, WriteResponse};
use lib::{AdvanceResult, FSMNodeWrapper, ToCSV, get_test_fsm};
use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::Write;
use std::io::read_to_string;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::exit;
use std::sync::Arc;
use std::sync::RwLock;
use std::{env, thread};

use bufstream::BufStream;
use lib::FSMCursor;
use lib::protocol::{Request, Response};

fn handle_request(
    req: Request,
    cursor: &mut FSMCursor,
    stream: &mut BufStream<UnixStream>,
) -> std::io::Result<()> {
    match req {
        Request::Revert => {
            cursor.revert();
            stream.write_response(Response::Expanded(cursor.input_buf()))?;
        }
        Request::Reset => {
            cursor.reset();
        }
        Request::Advance(str) => {
            str.chars().try_for_each(|c| match cursor.advancex(c) {
                Some(AdvanceResult::Expanded(s)) => Response::Expanded(&s).write(stream),
                Some(AdvanceResult::ExpandedAfterUserdef(s)) => {
                    Response::RegexFull.write(stream)?;
                    Response::Expanded(&s).write(stream)
                }
                Some(AdvanceResult::InvalidChar) => Response::InvalidChar.write(stream),
                Some(AdvanceResult::UserDefStarted) => Response::RegexStart.write(stream),
                None => Response::Ok.write(stream),
            })?;
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
    std::fs::remove_file(sock_addr).unwrap();
}

fn main() -> std::io::Result<()> {
    let sock_addr = get_sock_addr();
    // if the server panicked and didn't cleanup, do that now
    let _ = std::fs::remove_file(&sock_addr);
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

    let fsm_dir = env::var("NIGHTFURY_FSMDIR").unwrap_or("./nightfury_fsms".to_string());
    if std::path::Path::new(&fsm_dir).is_dir() {
        for fsm in read_dir(fsm_dir)? {
            match fsm {
                Ok(fsm) => {
                    let mut fsms = fsms.write().unwrap();
                    let file_name = fsm.file_name();
                    #[cfg(debug_assertions)]
                    if file_name == ".gitkeep" {
                        continue;
                    }
                    // if let Some(file_name) = file_name.to_str()
                    //     && let Ok(file) = File::open(&file_name)
                    //     && let Ok(csv) = read_to_string(file)
                    // {
                    //     fsms.insert(file_name.to_string(), FSMNodeWrapper::from_csv(&csv));
                    // }
                    match file_name.to_str() {
                        Some(fsm_name) => {
                            let mut fsm_name = fsm_name.to_string();
                            if fsm_name.ends_with(".fsm") {
                                fsm_name = fsm_name[..fsm_name.len() - 4].to_string();
                            }
                            println!("Loaded fsm '{fsm_name}'");
                            fsms.insert(
                                fsm_name,
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
                    let mut cursors = Vec::new();
                    let mut current_cursor = 0;
                    while let Ok(req) = stream.read_request(&mut buf) {
                        println!("req: {req:?}");
                        match req {
                            Request::Initialize(name)
                                if let Some(fsm) = fsms_clone.read().unwrap().get(name) =>
                            {
                                if cursors.len() == u8::MAX.into() {
                                    server_err(&mut stream, "Cursor limit exceeded")?;
                                    continue;
                                }
                                current_cursor = cursors.len();
                                cursors.push(FSMCursor::new(fsm));
                                Response::CursorHandle(cursors.len() as u8).write(&mut stream)?;
                            }
                            Request::Initialize(ref name) => {
                                server_err(&mut stream, &format!("Unknown language '{name}'"))?;
                            }
                            Request::GetCapabilities => {
                                Response::Capabilities(
                                    fsms_clone
                                        .read()
                                        .unwrap()
                                        .keys()
                                        .map(|s| s.as_str())
                                        .collect(),
                                )
                                .write(&mut stream)?;
                            }
                            Request::SetCursor(chandle) => {
                                if cursors.len() > chandle.into() {
                                    current_cursor = chandle.into();
                                } else {
                                    server_err(&mut stream, &format!("Invalid handle: {chandle}"))?;
                                }
                            }
                            _ if let Some(cursor) = cursors.get_mut(current_cursor) => {
                                handle_request(req, cursor, &mut stream)?
                            }
                            _ => server_err(
                                &mut stream,
                                &format!("Got {req:?} but don't have a cursor yet!"),
                            )?,
                        }
                        stream.flush().expect("stream flush");
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

fn server_err(stream: &mut BufStream<UnixStream>, errmsg: &str) -> std::io::Result<()> {
    eprintln!("{errmsg}");
    stream.write_err(errmsg)
}
