#![feature(let_chains)]

use std::io::{Read, Write};

use console::Term;
use lib::*;

fn main() {
    let root = TreeNode::new_keyword("BEGIN".to_string(), String::new());
    let child =
        TreeNode::new_keyword_with_parent("unsigned".to_string(), "u".to_string(), root.clone());
    let child2 =
        TreeNode::new_keyword_with_parent("signed".to_string(), "s".to_string(), root.clone());
    let types = TreeNode::new(NodeType::Null, &child);

    let int = TreeNode::new_keyword_with_parent("int".to_string(), "i".to_string(), types.clone());
    child.borrow_mut().add_child(&types);
    child2.borrow_mut().add_child(&types);

    let child = TreeNode::new(
        NodeType::UserDefined {
            final_chars: vec!['='],
        },
        &int,
    );
    let child = TreeNode::new(NodeType::Null, &child);

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
        // if cursor.is_in_userdefined_stage() {
        //     print!("{input}");
        // }
        std::io::stdout().flush();
    }
}
