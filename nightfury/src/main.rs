#![feature(let_chains)]

// use std::{fs::File, io::Read, path::Path};
use std::io::Write;

use console::Term;
use debug_print::debug_println;
use lib::*;

fn main() -> std::io::Result<()> {
    let ebnf = r##"
        select_statement ::= 'SELECT' select_list 'FROM' table_reference [ where_clause ] ';';
        select_list ::= "*" | ( column_name { "," column_name } );
        
        where_clause ::= "WHERE" condition;
        condition ::= expression logical_operator expression;
        comparison_operator ::= "=" | "!=" | "<" | ">" | "<=" | ">=";
        logical_operator ::= "AND" | "OR" | "NOT";
        expression ::= term { ( "+" | "-" ) term };
        term ::= factor { ( "*" | "/" ) factor };
        factor ::= column_name | value | "(" expression ")";
        value ::= string_literal | numeric_literal | boolean_literal | date_literal;
        
        table_reference ::= table_name [ alias ] { "," table_name [ alias ] };

        string_literal ::= "'" { character } "'";
        numeric_literal ::= digit { digit };
        boolean_literal ::= "TRUE" | "FALSE";
        date_literal ::= "'" date_string "'";
        date_string ::= digit { digit } "-" digit { digit } "-" digit { digit };
        
        alias ::= "AS" identifier;
        character ::= letter | digit | special_character;
        digit ::= #'[0-9] ';
        letter ::= #'[A-Za-z]';
        special_character ::= " " | "!" | "#" | "$" | "%" | "&" | "'" | "(" | ")" | "*" | "+" | "," | "-" | "." | "/" | ":" | ";" | "<" | "=" | ">" | "?" | "@" | "[" | "\\" | "]" | "^" | "_" | "{" | "|" | "}" | "~";
        column_name ::= identifier;
        table_name ::= identifier;
        identifier ::= #'[A-Za-z][0-9A-Za-z_]* ';
    "##;
    let ebnf = r"
        query ::= select | insert;
        select ::= 'SELECT' '*' | collist 'FROM' #'^.*;$';
        insert ::= 'INSERT INTO' #'^.* $' 'VALUES' '(' collist ')';
        collist ::= col ( ',' collist )?;
        col ::= #'^.*[, ]$';
    ";
    // let ebnf = r"
    //     test ::= { t2 } '~';
    //     t2 ::= 'ewe';
    // ";
    // read EBNF from file
    // let mut ebnf = String::new();
    // File::open(&Path::new("sql.ebnf"))
    //     .unwrap()
    //     .read_to_string(&mut ebnf)
    //     .unwrap();
    if let Ok(root) = frontend::create_graph_from_ebnf(&ebnf) {
        debug_println!("FSM:");
        root.borrow().dbg();
        debug_println!("FSM node cnt: {}", FSMNode::node_cnt(&root));
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
