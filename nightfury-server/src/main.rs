use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;

use lib::protocol::Request;
use lib::{FSMCursor, FSMNode};

fn handle_client(mut stream: UnixStream, cursor: FSMCursor) {
    let mut buf = String::new(); // bad
    stream.read_to_string(&mut buf).expect("buf read");
    let req: Request = serde_json::from_str(&buf).expect("Request deserialize");
    match req {
        Request::Init(lang) => {}
        Request::Reset => {}
        Request::GetCapabilities => {}
        Request::Advance(c) => {}
        Request::AdvanceStr(str) => {}
    }
}

fn main() -> std::io::Result<()> {
    let listener = UnixListener::bind("./nightfury.sock")?;
    let root = FSMNode::new_null(None);

    // accept connections and process them, spawning a new thread for each one
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                /* connection succeeded */
                let cursor = FSMCursor::new(&root);
                thread::spawn(|| handle_client(stream, cursor));
            }
            Err(err) => {
                /* connection failed */
                break;
            }
        }
    }
    Ok(())
}
