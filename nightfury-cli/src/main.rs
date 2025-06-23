use clap::Parser;
use clap::command;
use std::io::BufRead;
use std::os::unix::net::UnixStream;

use bufstream::BufStream;
use lib::protocol::Request;
use lib::protocol::WriteNullDelimitedExt;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// input to enter into fsm
    #[arg(short, long)]
    input: Option<String>,

    /// resets fsm
    #[arg(short, long)]
    reset: bool,

    /// path to nightfury socket
    #[arg()]
    sock_path: String,

    /// name of fsm
    #[arg(short, long)]
    name: String,

    /// list capabilities
    #[arg(short, long)]
    list: bool,
}

fn build_request(req: Request, stream: &mut BufStream<UnixStream>) -> std::io::Result<()> {
    let msg = serde_json::to_string(&req)?;
    stream.write_with_null_flush(msg.as_bytes())?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let stream = UnixStream::connect(args.sock_path)?;
    let mut stream = BufStream::new(stream);
    println!("Connected!");
    if args.list {
        build_request(Request::GetCapabilities, &mut stream)?;
    }

    build_request(Request::Init(args.name), &mut stream)?;
    if args.reset {
        build_request(Request::Reset, &mut stream)?;
    }
    if let Some(input) = &args.input
        && input.len() == 1
    {
        build_request(Request::Advance(input.chars().nth(0).unwrap()), &mut stream)?;
    }
    if let Some(input) = args.input {
        build_request(Request::AdvanceStr(input), &mut stream)?;
    }
    println!("Done writing");
    let mut response = Vec::new();
    stream.read_until(0, &mut response)?;
    println!("{:?}", str::from_utf8(&response[..&response.len() - 1]));
    Ok(())
}
