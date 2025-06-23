use std::io::BufRead;
use std::os::unix::net::UnixStream;

use bufstream::BufStream;
use lib::protocol::Request;
use lib::protocol::WriteNullDelimitedExt;

fn main() -> std::io::Result<()> {
    let stream =
        UnixStream::connect("/home/jason/clones/nightfury/nightfury-server/nightfury.sock")?;
    let mut stream = BufStream::new(stream);
    println!("Connected!");
    let msg = serde_json::to_string(&Request::GetCapabilities).unwrap();
    dbg!(&msg);
    stream.write_with_null_flush(msg.as_bytes())?;
    println!("Done writing");
    let mut response = Vec::new();
    stream.read_until(0, &mut response)?;
    println!("{:?}", str::from_utf8(&response[..&response.len() - 1]));
    Ok(())
}
