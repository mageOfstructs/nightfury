use std::io::{Read, Write};

use console::Term;
use lib::*;

fn main() {
    let root = TreeNode::new_keyword("BEGIN".to_string(), String::new());
    let child =
        TreeNode::new_keyword_with_parent("unsigned".to_string(), "u".to_string(), root.clone());
    let child =
        TreeNode::new_keyword_with_parent("int".to_string(), "i".to_string(), child.clone());
    let child = TreeNode::new(
        NodeType::UserDefined {
            final_chars: vec!['='],
        },
        &child,
    );
    let child = TreeNode::new(NodeType::Null, &child);

    root.borrow().dbg();
    let mut cursor = TreeCursor::new(&root);

    let terminal = Term::stdout();
    while !cursor.is_done() {
        let input = terminal.read_char().unwrap();
        println!("{input}");
        if let Some(res) = cursor.advance(input) {
            print!("{} ", res);
        }
        std::io::stdout().flush();
    }
}
