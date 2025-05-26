#![feature(let_chains)]

use std::io::Write;

use console::Term;
use debug_print::debug_println;
use lib::*;

fn main() -> std::io::Result<()> {
    let ebnf = r"
        insert_statement ::= 'INSERT INTO' col ( '(' col { ',' col } ')' )? 'VALUES' '(' col { ',' col } ')';
        col ::= #'^.*[, ]$';
    ";
    // read EBNF from file
    // let mut complete_ebnf = String::new();
    // File::open(&Path::new("sql.ebnf"))
    //     .unwrap()
    //     .read_to_string(&mut complete_ebnf)
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
