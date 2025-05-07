#![feature(let_chains)]

use std::io::Write;

use console::Term;
use lib::*;

fn main() {
    let root = TreeNode::new_keyword("BEGIN".to_string(), String::new());
    let mut sign_token = NodeValue {
        ntype: NodeType::Keyword {
            short: String::from("u"),
            expanded: String::from("unsigned"),
            closing_token: None,
        },
        optional: false,
    };
    let child = TreeNode::new(sign_token.clone(), &root);
    sign_token.ntype = NodeType::Keyword {
        short: String::from("s"),
        expanded: String::from("signed"),
        closing_token: None,
    };

    let child2 = TreeNode::new(sign_token, &root);
    let types = TreeNode::new_required(NodeType::Null, &child);

    let int = TreeNode::new_keyword_with_parent("int".to_string(), "i".to_string(), types.clone());
    let float =
        TreeNode::new_keyword_with_parent("short".to_string(), "s".to_string(), types.clone());

    let userdefined_node = TreeNode::new_required(
        NodeType::UserDefined {
            final_chars: vec!['='],
        },
        &int,
    );
    let null = TreeNode::new_required(NodeType::Null, &userdefined_node);
    float.borrow_mut().add_child(&userdefined_node);

    child2.borrow_mut().add_child(&types);
    root.borrow_mut().add_child(&types);

    let expression = TreeNode::new(
        NodeValue {
            ntype: NodeType::Keyword {
                short: String::from("("),
                expanded: String::from("("),
                closing_token: Some(String::from(")")),
            },
            optional: false,
        },
        &root,
    );
    let expr_boolvar = TreeNode::new(
        NodeValue {
            ntype: NodeType::UserDefined {
                final_chars: vec![')', '&', '('],
            },
            optional: false,
        },
        &expression,
    );
    expr_boolvar.borrow_mut().add_child(&null.clone());
    let cond_and = TreeNode::new(
        NodeValue {
            ntype: NodeType::Keyword {
                short: String::from("&"),
                expanded: String::from("&&"),
                closing_token: None,
            },
            optional: false,
        },
        &expression,
    );
    cond_and.borrow_mut().add_child(&expr_boolvar);
    expr_boolvar.borrow_mut().add_child(&cond_and);
    expr_boolvar.borrow_mut().add_child(&expression);

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
