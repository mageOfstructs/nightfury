use clap::Parser;
use clap::Subcommand;
use clap::command;
use console::Term;
use lib::FSMCursor;
use lib::FSMNodeWrapper;
use lib::ToCSV;
use lib::frontend::create_graph_from_ebnf;
use std::fs::File;
use std::io::BufRead;
use std::io::Write;
use std::io::read_to_string;
use std::io::stdin;

use bufstream::BufStream;
use lib::protocol::{Request, Sender};

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
    /// WIP: (debug) send requests to a server instance; NOTE: there is currently no way of retaining state
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
    /// Debug: print fsm
    Dbg {
        /// path of nightfury fsm file
        fsm_path: String,
    },
    Chat {
        fsm_path: String,
    },
}

fn send_request(req: Request, stream: &mut BufStream<Sender>) -> std::io::Result<()> {
    req.write(stream)?;
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
                    let out = out.as_ref().map_or("./nightfury.fsm", |s| s);
                    let root = create_graph_from_ebnf(&ebnf);
                    match root {
                        Ok(root) => {
                            let out_file = File::create_new(out);
                            match out_file {
                                Ok(mut out_file) => {
                                    out_file.write_all(root.to_csv().as_bytes())?;
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
            let stream = Sender::connect(sock_path)?;
            let mut stream = BufStream::new(stream);
            println!("Connected!");
            if list {
                send_request(Request::GetCapabilities, &mut stream)?;
            }

            if let Some(name) = name {
                send_request(Request::Initialize(&name), &mut stream)?;
                stream.flush()?;
                stream.read_until(0, &mut Vec::new())?;
                if reset {
                    send_request(Request::Reset, &mut stream)?;
                }
                if let Some(input) = &input {
                    send_request(Request::Advance(input), &mut stream)?;
                }
            }
            stream.flush()?;
            let mut response = Vec::new();
            stream.read_until(0, &mut response)?;
            println!("{:?}", str::from_utf8(&response[..&response.len() - 1]));
        }
        NightfurySubcommand::Dbg { fsm_path } => {
            let fsm = FSMNodeWrapper::from_csv_file(&fsm_path);
            match fsm {
                Ok(fsm) => {
                    println!("FSM:");
                    fsm.borrow().dbg();
                }
                Err(err) => eprintln!("{err}"),
            }
        }
        NightfurySubcommand::Chat { fsm_path } => {
            let fsm = FSMNodeWrapper::from_csv_file(&fsm_path);
            match fsm {
                Ok(root) => {
                    println!("FSM:");
                    root.borrow().dbg();
                    let mut cursor = FSMCursor::new(&root);

                    let terminal = Term::stdout();
                    while !cursor.is_done() {
                        let input = terminal.read_char().unwrap();
                        match input {
                            '\x08' => cursor.clear_inputbuf(),
                            _ => {
                                if let Some(res) = cursor.advance(input) {
                                    print!("{res} ");
                                }
                            }
                        }
                        if cursor.is_in_userdefined_stage() {
                            print!("{input}");
                        }
                        std::io::stdout().flush()?;
                    }
                }
                Err(err) => eprintln!("{err}"),
            }
        }
    }

    Ok(())
}
