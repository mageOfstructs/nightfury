#![feature(let_chains)]

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use console::Term;
use lib::{frontend::print_parsed_ebnf, *};
use regex::Regex;

fn main() {
    let sql = r"
        query ::= select | insert;
        select ::= 'SELECT' '*' | collist 'FROM' #'^.*;$';
        insert ::= 'INSERT INTO' #'^.* $' 'VALUES' '(' collist ')';
        collist ::= col ( ',' collist )?;
        col ::= #'^.*[, ]$';
    ";
    // Repeat nodes:
    // identifier ::= letter { letter | digit | "_" };
    // let mut complete_ebnf = String::new();
    // File::open(&Path::new("sql.ebnf"))
    //     .unwrap()
    //     .read_to_string(&mut complete_ebnf)
    //     .unwrap();
    // print_parsed_ebnf(&complete_ebnf);
    if let Ok(root) = frontend::create_graph_from_ebnf(&sql) {
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
                        print!("{} ", res);
                    }
                }
            }
            if cursor.is_in_userdefined_stage() {
                print!("{input}");
            }
            std::io::stdout().flush();
        }
    } else {
        eprintln!("Error while creating graph");
    }
    return;
    let root = FSMNode::new_null(None);
    let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
    let child = FSMNode::new(sign_token.clone(), &root);
    sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

    let signed = FSMNode::new(sign_token, &root);
    let types = FSMNode::new_required(NodeType::Null, &child);

    let int = FSMNode::new_keyword_with_parent("int".to_string(), types.clone());
    let short = FSMNode::new_keyword_with_parent("short".to_string(), types.clone());
    let short2 = FSMNode::new_keyword_with_parent("shark".to_string(), types.clone());

    let userdefined_node = FSMNode::new_required(
        NodeType::UserDefinedRegex(Regex::new("[0-9]{3,3}").unwrap()),
        &int,
    );
    // let userdefined_node = TreeNode::new_required(
    //     NodeType::UserDefined {
    //         final_chars: vec!['='],
    //     },
    //     &int,
    // );
    let null = FSMNode::new_required(NodeType::Null, &userdefined_node);
    short.borrow_mut().add_child(&userdefined_node);

    signed.borrow_mut().add_child(&types);
    root.borrow_mut().add_child(&types);

    let expression = FSMNode::new(
        NodeType::Keyword(Keyword::new("(".to_string(), Some(")".to_string()))),
        &root,
    );
    let expr_boolvar = FSMNode::new(
        NodeType::UserDefined {
            final_chars: vec![')', '&', '('],
        },
        &expression,
    );
    expr_boolvar.borrow_mut().add_child(&null.clone());
    let cond_and = FSMNode::new(
        NodeType::Keyword(Keyword::new("&&".to_string(), None)),
        &expression,
    );
    cond_and.borrow_mut().add_child(&expr_boolvar);
    expr_boolvar.borrow_mut().add_child(&cond_and);
    expr_boolvar.borrow_mut().add_child(&expression);

    println!("Dump:");
    int.borrow().dump_children();
    // root.borrow().dbg();
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
        std::io::stdout().flush();
    }
}
