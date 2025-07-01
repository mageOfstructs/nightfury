use clap::Parser;
use clap::Subcommand;
use clap::command;
use lib::ToCSV;
use lib::frontend::create_graph_from_ebnf;
use std::fs::File;
use std::io::BufRead;
use std::io::Write;
use std::io::read_to_string;
use std::io::stdin;
use std::os::unix::net::UnixStream;

use bufstream::BufStream;
use lib::protocol::Request;
use lib::protocol::WriteNullDelimitedExt;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: NightfurySubcommand,
}

#[derive(Subcommand, Debug)]
enum NightfurySubcommand {
    /// generates an FSM from a provided ebnf
    Generate {
        // file to read ebnf from, use stdin if ommited
        path: Option<String>,
        #[arg(short, long)]
        out: Option<String>,
    },
    /// (debug) send requests to a server instance; NOTE: there is currently no way of retaining state
    /// between calls.
    Send {
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
        sock_path: String,
    },
}

fn send_request(req: Request, stream: &mut BufStream<UnixStream>) -> std::io::Result<()> {
    let msg = serde_json::to_string(&req)?;
    dbg!(&msg);
    stream.write_with_null_flush(msg.as_bytes())?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    match args.command {
        NightfurySubcommand::Generate { path, out } => {
            let ebnf = match path {
                Some(path) => std::fs::read_to_string(path),
                None => read_to_string(stdin()),
            };
            match ebnf {
                Ok(ebnf) => {
                    let out = out.as_ref().map_or("./nightfury.fsm", |s| &s);
                    let root = create_graph_from_ebnf(&ebnf);
                    match root {
                        Ok(root) => {
                            let out_file = File::create_new(out);
                            match out_file {
                                Ok(mut out_file) => {
                                    out_file.write_all(&root.to_csv().as_bytes())?;
                                }
                                Err(e) => eprintln!("{e}"),
                            }
                        }
                        Err(err) => {
                            eprintln!("Error creating ebnf:");
                            eprintln!("{err}");
                        }
                    }
                }
                Err(e) => eprintln!("Error reading ebnf: {e}"),
            }
        }
        NightfurySubcommand::Send {
            input,
            reset,
            name,
            list,
            sock_path,
        } => {
            let stream = UnixStream::connect(sock_path)?;
            let mut stream = BufStream::new(stream);
            println!("Connected!");
            if list {
                send_request(Request::GetCapabilities, &mut stream)?;
            }

            if let Some(name) = name {
                send_request(Request::Init(name), &mut stream)?;
                if reset {
                    send_request(Request::Reset, &mut stream)?;
                }
                if let Some(input) = &input
                    && input.len() == 1
                {
                    send_request(Request::Advance(input.chars().nth(0).unwrap()), &mut stream)?;
                } else if let Some(input) = input {
                    send_request(Request::AdvanceStr(input), &mut stream)?;
                }
            }
            let mut response = Vec::new();
            stream.read_until(0, &mut response)?;
            println!("{:?}", str::from_utf8(&response[..&response.len() - 1]));
        }
    }

    Ok(())
}
