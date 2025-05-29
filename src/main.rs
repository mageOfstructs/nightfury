#![feature(let_chains)]

use std::{fs::File, io::Read, io::Write, path::Path};

use console::Term;
use debug_print::debug_println;
use lib::*;

fn main() -> std::io::Result<()> {
    let ebnf = r#"
        select_statement ::= 'SELECT' select_list 'FROM' table_reference ';';
        select_list ::= "*" | ( column_name { "," column_name } );
        
        table_reference ::= table_name [ alias ] { "," table_name [ alias ] };
        
        alias ::= "AS" identifier;
        column_name ::= identifier;
        table_name ::= identifier;
        identifier ::= #'[A-Za-z][0-9A-Za-z_]* ';
    "#;
    // read EBNF from file
    // let mut ebnf = String::new();
    // File::open(&Path::new("sql.ebnf"))
    //     .unwrap()
    //     .read_to_string(&mut ebnf)
    //     .unwrap();
    if let Ok(root) = frontend::create_graph_from_ebnf(&ebnf) {
        debug_println!("FSM:");
        root.borrow().dbg();
        let mut cursor = FSMCursor::new(&root);

        let terminal = Term::stdout();
        while !cursor.is_done() {
            let input = terminal.read_char().unwrap();
            match input {
                '\x08' => cursor.clear_inputbuf(),
                _ => {
                    if let Some(res) = cursor.advance(input) {
                        print!("{} ", res);
                    }
                }
            }
            if cursor.is_in_userdefined_stage() {
                print!("{input}");
            }
            std::io::stdout().flush()?;
        }
    } else {
        eprintln!("Error while creating graph");
    }
    Ok(())
}
