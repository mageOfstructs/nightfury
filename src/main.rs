use std::io::{Read, Write};

use console::Term;
use lib::*;

fn main() {
    let root = TreeNode::new("BEGIN".to_string(), String::new());
    let child = TreeNode::new_with_parent("unsigned".to_string(), "u".to_string(), root.clone());
    let second = TreeNode::new_with_parent("int".to_string(), "i".to_string(), child.clone());
    TreeNode::new_with_parent("asdf".to_string(), "a".to_string(), second.clone());
    let mut cursor = TreeCursor::new(&root);
    root.borrow().dbg();

    let terminal = Term::stdout();
    while !cursor.is_done() {
        let input = terminal.read_char().unwrap();
        cursor.advance(input);
        print!("{} ", cursor.get_last_matched_node());
        std::io::stdout().flush();
    }
}
