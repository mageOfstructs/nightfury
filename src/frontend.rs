use std::{cell::RefCell, collections::HashMap, rc::Rc};

use debug_print::debug_println;
use ebnf::{Expression, Grammar, Node, RegexExtKind, SymbolKind};
use regex::Regex;

use crate::TreeNode;

pub fn print_parsed_ebnf(syntax: &str) {
    let grammar = ebnf::get_grammar(&syntax).unwrap();
    for node in grammar.expressions {
        println!("{:?}", node);
    }
}

enum TerminalState {
    Stub,
    Created,
}

fn handle_node(
    grammar: &Grammar,
    cur_node: &Node,
    cur_root: &Rc<RefCell<TreeNode>>,
    terminals: &mut HashMap<String, (Rc<RefCell<TreeNode>>, TerminalState)>,
) -> Rc<RefCell<TreeNode>> {
    debug_println!("handle_node got {:?}", cur_node);
    let ret = match &cur_node {
        Node::String(str) => {
            TreeNode::new_keyword_with_parent(str.to_string(), Rc::clone(cur_root))
        }
        Node::RegexString(r) => TreeNode::new(
            crate::NodeType::UserDefinedRegex(Regex::new(r).unwrap()),
            &cur_root,
        ),
        Node::Terminal(name) => {
            if terminals.contains_key(name) {
                debug_println!("Found {name} in cache!");
                // let term_clone = Rc::new(RefCell::new(
                //     terminals.get(name).unwrap().borrow().deep_clone(),
                // ));
                let term = terminals.get(name).unwrap();
                let term_clone = match term.1 {
                    TerminalState::Stub => term.0.clone(),
                    TerminalState::Created => term.0.borrow().deep_clone(),
                };
                println!("cur_root:");
                cur_root.borrow().dbg();
                println!("term:");
                term_clone.borrow().dbg();
                TreeNode::add_child_cycle_safe(cur_root, &term_clone);
                println!("after add:");
                cur_root.borrow().dbg();
                term_clone
            } else {
                println!("Creating terminal {name}...");
                let terminal = find_terminal(&grammar, &name);
                if terminal.is_none() {
                    panic!("Terminal reference '{name}' not found!");
                }
                let terminal = terminal.unwrap();
                let term_root = TreeNode::new_null(None);
                terminals.insert(
                    name.to_string(),
                    (Rc::clone(&term_root), TerminalState::Stub),
                );
                debug_println!("Creating terminal...");
                handle_node(grammar, &terminal.rhs, &term_root, terminals);
                terminals.insert(
                    name.to_string(),
                    (Rc::clone(&term_root), TerminalState::Created),
                );
                debug_println!("Finish terminal");
                println!("young {}:", name);
                term_root.borrow().dbg();
                let ret = term_root.borrow().deep_clone();
                TreeNode::add_child_cycle_safe(cur_root, &ret);
                ret
            }
        }
        Node::Multiple(nodes) => {
            let mut cur_treenode = cur_root.clone();
            // TODO: this doesn't handle multiple Optionals in a row!!! Make this a Vec instead
            let mut last_opt: Option<Rc<RefCell<TreeNode>>> = None;
            nodes.iter().for_each(|node| {
                debug_println!("Multiple at {node:?}");
                let tree_bit = handle_node(grammar, &node, &cur_treenode, terminals);
                if let Some(last_opt) = &last_opt {
                    TreeNode::add_child_to_all_leaves(&last_opt, &tree_bit);
                    // yes this needs to be here
                    last_opt.borrow().handle_potential_conflict(&tree_bit);
                }
                match node {
                    Node::RegexExt(_, RegexExtKind::Optional)
                    | Node::Optional(_)
                    | Node::Repeat(_) => {
                        last_opt = Some(tree_bit);
                    }
                    _ => {
                        last_opt = None;
                        // FIXME: this can lead us astray if the we merged an already used Terminal into our path
                        cur_treenode = tree_bit.borrow().race_to_leaf().unwrap_or(tree_bit.clone());
                    }
                }
            });
            cur_treenode
        }
        Node::RegexExt(node, RegexExtKind::Optional) => {
            handle_node(grammar, &node, cur_root, terminals)
        }
        Node::Optional(node) => handle_node(grammar, &node, cur_root, terminals),
        Node::Symbol(n1, SymbolKind::Concatenation, n2) => {
            let t1 = handle_node(grammar, &n1.to_owned(), &cur_root, terminals);
            let t2 = handle_node(grammar, &n2.to_owned(), &t1, terminals);
            t1
        }
        Node::Symbol(n1, SymbolKind::Alternation, n2) => {
            let root = TreeNode::new_null(Some(cur_root));
            let t1 = handle_node(grammar, &n1.to_owned(), &root, terminals);
            let t2 = handle_node(grammar, &n2.to_owned(), &root, terminals);
            let child = TreeNode::new_null(None);
            TreeNode::add_child_to_all_leaves(&t1, &child);
            TreeNode::add_child_to_all_leaves(&t2, &child);
            root
        }
        Node::Group(node) => handle_node(grammar, node, cur_root, terminals),
        Node::Repeat(node) => {
            // FIXME: repeats can apparently also mean 0
            let subroot = handle_node(grammar, &node, cur_root, terminals);
            TreeNode::add_child_cycle_safe(&subroot, &subroot);
            // subroot.borrow().add_child(&subroot); // this will crash so hard
            subroot
            // panic!("We got a Repeat node! go look at the bnf and see what it's supposed to be")
        }
        _ => {
            println!("Unimplemented: {cur_node:?}");
            todo!()
        }
    };
    println!("cur_root:");
    cur_root.borrow().dbg();
    ret
}

fn find_terminal<'a>(grammer: &'a Grammar, name: &'a str) -> Option<&'a Expression> {
    grammer.expressions.iter().find(|expr| expr.lhs == name)
}

pub fn create_graph_from_ebnf(ebnf: &str) -> Result<Rc<RefCell<TreeNode>>, String> {
    match ebnf::get_grammar(ebnf) {
        Ok(grammar) => {
            let root = TreeNode::new_null(None);
            let root_node = grammar.expressions.get(0).expect("Empty BNF!");
            let mut terminals = HashMap::new();
            handle_node(
                &grammar,
                &Node::Terminal(root_node.lhs.to_owned()),
                &root,
                &mut terminals,
            );
            // sanity op, is_done() won't cancel preemptively
            TreeNode::add_child_to_all_leaves(&root, &TreeNode::new_null(None));
            for (name, term) in terminals.iter() {
                println!("Term {}", name);
                term.0.borrow().dbg();
            }
            Ok(root)
        }
        Err(err) => Err(err.to_string()),
    }
}
