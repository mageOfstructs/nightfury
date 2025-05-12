use std::{cell::RefCell, collections::HashMap, rc::Rc};

use ebnf::{Expression, Grammar, Node, RegexExtKind, SymbolKind};
use regex::Regex;

use crate::TreeNode;

pub fn do_stuff(syntax: &str) {
    let grammar = ebnf::get_grammar(&syntax).unwrap();
    for node in grammar.expressions {
        println!("{:?}", node);
    }
}

fn handle_node(
    grammar: &Grammar,
    cur_node: &Node,
    cur_root: &Rc<RefCell<TreeNode>>,
) -> Rc<RefCell<TreeNode>> {
    println!("{cur_node:?}");
    match &cur_node {
        Node::String(str) => {
            TreeNode::new_keyword_with_parent(str.to_string(), Rc::clone(cur_root))
        }
        Node::RegexString(r) => TreeNode::new(
            crate::NodeType::UserDefinedRegex(Regex::new(r).unwrap()),
            &cur_root,
        ),
        Node::Terminal(name) => {
            let terminal = find_terminal(&grammar, &name).expect("Terminal reference not found!");
            handle_node(grammar, &terminal.rhs, cur_root)
        }
        Node::Multiple(nodes) => {
            let mut cur_treenode = cur_root.clone();
            let mut last_opt: Option<Rc<RefCell<TreeNode>>> = None;
            nodes.iter().for_each(|node| {
                let tree_bit =
                    handle_node(grammar, &node, &TreeNode::new_null(Some(&cur_treenode)));
                if let Some(last_opt) = &last_opt {
                    last_opt.borrow_mut().add_child(&tree_bit);
                }
                match node {
                    Node::RegexExt(_, RegexExtKind::Optional) | Node::Optional(_) => {
                        last_opt = Some(tree_bit);
                    }
                    _ => {
                        cur_treenode = tree_bit;
                    }
                }
            });
            cur_treenode
        }
        Node::RegexExt(node, RegexExtKind::Optional) => handle_node(grammar, &node, cur_root),
        Node::Optional(node) => handle_node(grammar, &node, cur_root),
        Node::Symbol(n1, SymbolKind::Concatenation, n2) => {
            let t1 = handle_node(grammar, &n1.to_owned(), &cur_root);
            let t2 = handle_node(grammar, &n2.to_owned(), &t1);
            t2
        }
        Node::Symbol(n1, SymbolKind::Alternation, n2) => {
            let t1 = handle_node(grammar, &n1.to_owned(), &cur_root);
            let t2 = handle_node(grammar, &n2.to_owned(), &cur_root);
            let child = TreeNode::new_null(Some(&t1));
            child.borrow_mut().add_child(&t2);
            child
        }
        Node::Group(node) => handle_node(grammar, node, cur_root),
        Node::Repeat(_) => {
            panic!("We got a Repeat node! go look at the bnf and see what it's supposed to be")
        }
        _ => {
            println!("{cur_node:?}");
            todo!()
        }
    }
}

fn find_terminal<'a>(grammer: &'a Grammar, name: &'a str) -> Option<&'a Expression> {
    grammer.expressions.iter().find(|expr| expr.lhs == name)
}

pub fn create_graph_from_ebnf(ebnf: &str) -> Result<Rc<RefCell<TreeNode>>, String> {
    match ebnf::get_grammar(ebnf) {
        Ok(mut grammar) => {
            let root = TreeNode::new_null(None);
            let root_node = grammar.expressions.remove(0); // .expect("Empty BNF!");
            handle_node(&grammar, &root_node.rhs, &root);
            Ok(root)
        }
        Err(err) => Err(err.to_string()),
    }
}
