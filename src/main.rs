#![feature(let_chains)]

use std::io::Write;

use console::Term;
use lib::{frontend::do_stuff, *};
use regex::Regex;

fn main() {
    // let ebnf = r"
    //     syntax ::= ( signed_keyword )? types value;
    //     signed_keyword ::= 'signed' | 'unsigned';
    //     types ::= 'int' | 'short';
    //     value ::= #'^.+;$';
    // ";
    let ebnf = r"
        list ::= #'[0-9]' ( ',' list )?;
    ";
    let ebnf = r"
        query ::= select | insert;
        select ::= 'SELECT' collist 'FROM' #'^.*;$';
        insert ::= 'INSERT INTO' #'^.* $' 'VALUES' '(' collist2 ')';
        collist ::= col ( ',' collist )?;
        collist2 ::= col2 ( ',' collist2 )?;
        col ::= #'^.*[, ]$' | '*';
        col2 ::= #'^.*[, ]$' | '*';
    ";
    do_stuff(ebnf);
    if let Ok(root) = frontend::create_graph_from_ebnf(ebnf) {
        root.borrow().dbg();
        let mut cursor = TreeCursor::new(&root);

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
    let root = TreeNode::new_null(None);
    let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
    let child = TreeNode::new(sign_token.clone(), &root);
    sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

    let signed = TreeNode::new(sign_token, &root);
    let types = TreeNode::new_required(NodeType::Null, &child);

    let int = TreeNode::new_keyword_with_parent("int".to_string(), types.clone());
    let short = TreeNode::new_keyword_with_parent("short".to_string(), types.clone());
    let short2 = TreeNode::new_keyword_with_parent("shark".to_string(), types.clone());

    let userdefined_node = TreeNode::new_required(
        NodeType::UserDefinedRegex(Regex::new("[0-9]{3,3}").unwrap()),
        &int,
    );
    // let userdefined_node = TreeNode::new_required(
    //     NodeType::UserDefined {
    //         final_chars: vec!['='],
    //     },
    //     &int,
    // );
    let null = TreeNode::new_required(NodeType::Null, &userdefined_node);
    short.borrow_mut().add_child(&userdefined_node);

    signed.borrow_mut().add_child(&types);
    root.borrow_mut().add_child(&types);

    let expression = TreeNode::new(
        NodeType::Keyword(Keyword::new("(".to_string(), Some(")".to_string()))),
        &root,
    );
    let expr_boolvar = TreeNode::new(
        NodeType::UserDefined {
            final_chars: vec![')', '&', '('],
        },
        &expression,
    );
    expr_boolvar.borrow_mut().add_child(&null.clone());
    let cond_and = TreeNode::new(
        NodeType::Keyword(Keyword::new("&&".to_string(), None)),
        &expression,
    );
    cond_and.borrow_mut().add_child(&expr_boolvar);
    expr_boolvar.borrow_mut().add_child(&cond_and);
    expr_boolvar.borrow_mut().add_child(&expression);

    println!("Dump:");
    int.borrow().dump_children();
    // root.borrow().dbg();
    let mut cursor = TreeCursor::new(&root);

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
