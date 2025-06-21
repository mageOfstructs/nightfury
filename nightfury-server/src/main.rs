#![feature(if_let_guard)]
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;

use lib::protocol::Request;
use lib::{FSMCursor, FSMNode};

fn handle_client(req: Request, cursor: &mut FSMCursor) {
    match req {
        Request::Reset => {}
        Request::Advance(c) => {}
        Request::AdvanceStr(str) => {}
        _ => unreachable!(),
    }
}

fn main() -> std::io::Result<()> {
    let listener = UnixListener::bind("./nightfury.sock")?;
    let mut handles = Vec::new();
    let fsms = Arc::new(RwLock::new(HashMap::new()));
    fsms.write()
        .unwrap()
        .insert("test", FSMNode::new_null(None));

    // accept connections and process them, spawning a new thread for each one
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                /* connection succeeded */
                let fsms_clone = Arc::clone(&fsms);
                handles.push(thread::spawn(move || {
                    let mut buf = String::new(); // bad
                    let mut cursor = None;
                    while stream.read_to_string(&mut buf).expect("buf read") != 0 {
                        let req: Request = serde_json::from_str(&buf).expect("deserde");
                        match req {
                            Request::Init(ref name)
                                if let Some(fsm) =
                                    fsms_clone.read().unwrap().get(&name.as_str()) =>
                            {
                                cursor = Some(FSMCursor::new(fsm))
                            }
                            _ => {
                                handle_client(req, &mut cursor.as_mut().expect("didn't call init!"))
                            }
                        }
                    }
                    stream
                        .shutdown(std::net::Shutdown::Both)
                        .expect("stream shutdown");
                }));
            }
            Err(err) => {
                /* connection failed */
                break;
            }
        }
    }
    handles.into_iter().for_each(|h| {
        h.join().unwrap();
    });
    Ok(())
}
