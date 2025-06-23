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

    /// name of fsm
    #[arg(short, long)]
    name: Option<String>,

    /// list capabilities
    #[arg(short, long)]
    list: bool,

    /// path to nightfury socket
    #[arg()]
    sock_path: String,
}

fn send_request(req: Request, stream: &mut BufStream<UnixStream>) -> std::io::Result<()> {
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
        send_request(Request::GetCapabilities, &mut stream)?;
    }

    if let Some(name) = args.name {
        send_request(Request::Init(name), &mut stream)?;
        if args.reset {
            send_request(Request::Reset, &mut stream)?;
        }
        if let Some(input) = &args.input
            && input.len() == 1
        {
            send_request(Request::Advance(input.chars().nth(0).unwrap()), &mut stream)?;
        }
        if let Some(input) = args.input {
            send_request(Request::AdvanceStr(input), &mut stream)?;
        }
    }
    let mut response = Vec::new();
    stream.read_until(0, &mut response)?;
    println!("{:?}", str::from_utf8(&response[..&response.len() - 1]));
    Ok(())
}
