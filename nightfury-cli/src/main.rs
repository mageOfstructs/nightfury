use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;

use lib::protocol::Request;

fn main() -> std::io::Result<()> {
    let mut stream =
        UnixStream::connect("/home/dragon/clones/nightfury/nightfury-server/nightfury.sock")?;
    stream.write_all(
        serde_json::to_string(&Request::GetCapabilities)
            .unwrap()
            .as_bytes(),
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    println!("{response}");
    Ok(())
}
