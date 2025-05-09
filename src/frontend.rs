use std::{cell::RefCell, collections::HashMap, rc::Rc};

use ebnf::{Expression, Grammar, Node, SymbolKind};

use crate::TreeNode;

pub fn do_stuff(syntax: &str) {
    let grammar = ebnf::get_grammar(&syntax).unwrap();
    let mut hasmap_keywords = HashMap::new();
    let mut hashmap_terminals = HashMap::new();
    for node in grammar.expressions {
        println!("{:?}", node);
        match node.rhs {
            Node::String(_) | Node::RegexString(_) => {
                hasmap_keywords.insert(node.lhs, node.rhs);
            }
            Node::Terminal(ref str) => {
                hashmap_terminals.insert(str.clone(), create_tree(&node.rhs));
            }
            _ => {}
        }
    }
    println!("{hasmap_keywords:?}");
}

fn create_tree(root_node: &Node) -> Rc<RefCell<TreeNode>> {
    let root_tree = TreeNode::new_null(None);
    root_tree
}

fn handle_node(grammer: &Grammar, cur_node: &Expression, cur_root: Rc<RefCell<TreeNode>>) {
    match &cur_node.rhs {
        Node::String(str) => {
            TreeNode::new_keyword_with_parent(str.to_string(), cur_root);
        }
        Node::RegexString(_) | Node::RegexExt(..) => unimplemented!(),
        Node::Terminal(name) => {
            let terminal = find_terminal(&grammer, &name).expect("Terminal reference not found!");
            handle_node(grammer, terminal, cur_root);
        }
        Node::Multiple(nodes) => {
            let mut cur_node = cur_root;
            nodes.iter().for_each(|node| {
                let tree_bit = create_tree(&node);
                cur_node.borrow_mut().add_child(&tree_bit);
                // FIXME: make this line compile
                // cur_node = cur_node.borrow().get_last_child().unwrap().clone();
            });
        }
        Node::Optional(node) => {}
        _ => todo!(),
    }
}

fn find_terminal<'a>(grammer: &'a Grammar, name: &'a str) -> Option<&'a Expression> {
    grammer.expressions.iter().find(|expr| expr.lhs == name)
}
