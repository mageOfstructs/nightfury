use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;

use lib::protocol::Request;
use serde::Deserialize;

fn handle_client(mut stream: UnixStream) {
    let mut buf = String::new(); // bad
    stream.read_to_string(&mut buf).expect("buf read");
    let req: Request = serde_json::from_str(&buf).expect("Request deserialize");
}

fn main() -> std::io::Result<()> {
    let listener = UnixListener::bind("./nightfury.sock")?;

    // accept connections and process them, spawning a new thread for each one
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                /* connection succeeded */
                thread::spawn(|| handle_client(stream));
            }
            Err(err) => {
                /* connection failed */
                break;
            }
        }
    }
    Ok(())
}
