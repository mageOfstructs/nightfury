use std::collections::HashMap;

use debug_print::debug_println;
use ebnf::{Expression, Grammar, Node, RegexExtKind, SymbolKind};
use regex::Regex;

use super::FSMLock;
use super::FSMRc;
use crate::FSMNode;

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
    grammar: &mut Grammar,
    cur_node: Node,
    cur_root: &FSMRc<FSMLock<FSMNode>>,
    terminals: &mut HashMap<String, (FSMRc<FSMLock<FSMNode>>, TerminalState)>,
) -> FSMRc<FSMLock<FSMNode>> {
    debug_println!("handle_node got {:?}", cur_node);
    let ret = match cur_node {
        Node::String(str) => {
            FSMNode::new_keyword_with_parent(str.to_string(), FSMRc::clone(cur_root))
        }
        Node::RegexString(r) => FSMNode::new_userdef(Regex::new(&r).unwrap(), cur_root),
        Node::Terminal(name) => {
            if terminals.contains_key(&name) {
                debug_println!("Found {name} in cache!");
                let term = terminals.get(&name).unwrap();
                let term_clone = match term.1 {
                    TerminalState::Stub => term.0.clone(),
                    TerminalState::Created => term.0.borrow().deep_clone(),
                };
                debug_println!("linking back to {}", term_clone.borrow().short_id());
                FSMNode::add_child_cycle_safe(cur_root, &term_clone);
                debug_println!("after add:");
                cur_root.borrow().dbg();
                term_clone
            } else {
                debug_println!("Creating terminal {name}...");
                let terminal =
                    find_terminal(&grammar, &name).expect("Terminal reference '{name}' not found!");
                let term_root = FSMNode::new_null(None);
                debug_println!("term_root: {}", term_root.borrow().short_id());
                terminals.insert(
                    name.to_string(),
                    (FSMRc::clone(&term_root), TerminalState::Stub),
                );
                handle_node(grammar, terminal.rhs.clone(), &term_root, terminals);
                debug_println!("Finish terminal");
                debug_println!("young {}:", name);
                term_root.borrow().dbg();
                terminals.insert(name, (FSMRc::clone(&term_root), TerminalState::Created));
                let ret = term_root.borrow().deep_clone();
                FSMNode::add_child_cycle_safe(cur_root, &ret);
                ret
            }
        }
        Node::Multiple(nodes) => {
            let mut cur_treenode = cur_root.clone();
            nodes.into_iter().for_each(|node| {
                debug_println!("Multiple at {node:?}");
                let tree_bit = handle_node(grammar, node, &cur_treenode, terminals);
                debug_println!("Multiple got back:");
                tree_bit.borrow().dbg();
                // NOTE: this will only work as long as the other node handlers nicely merge their
                // diverging branches back into one single Null leaf
                cur_treenode = tree_bit.borrow().race_to_leaf().unwrap_or(tree_bit.clone());
                debug_println!("cur_treenode now at: {}", cur_treenode.borrow().short_id());
            });
            cur_treenode
        }
        Node::RegexExt(node, RegexExtKind::Optional) | Node::Optional(node) => {
            let tree_bit = handle_node(grammar, *node, cur_root, terminals);
            let dummy = FSMNode::new_null(None);
            FSMNode::add_child_to_all_leaves(&tree_bit, &dummy);
            FSMNode::add_child_cycle_safe(&cur_root, &dummy);
            tree_bit
        }
        Node::Symbol(n1, SymbolKind::Concatenation, n2) => {
            let t1 = handle_node(grammar, *n1, &cur_root, terminals);
            let _ = handle_node(grammar, *n2, &t1, terminals);
            t1
        }
        Node::Symbol(n1, SymbolKind::Alternation, n2) => {
            let root = FSMNode::new_null(Some(cur_root)); // need this here otherwise minify will
            // crash?
            let _ = handle_node(grammar, *n1, &root, terminals);
            let _ = handle_node(grammar, *n2, &root, terminals);
            let child = FSMNode::new_null(None);
            debug_println!("Alternation dummy child: {}", child.borrow().short_id());
            FSMNode::add_child_to_all_leaves(&root, &child);
            debug_println!("Finished alternation:");
            root.borrow().dbg();
            root
        }
        Node::Group(node) => handle_node(grammar, *node, cur_root, terminals),
        Node::Repeat(node) => {
            // need to guarantee this is a null so search_rec won't prematurely stop, e.g. when
            // cur_root is a Keyword
            let dummy_parent = FSMNode::new_null(Some(&cur_root));
            let subroot = handle_node(grammar, *node, &dummy_parent, terminals);

            let dummy = FSMNode::new_null(None);
            debug_println!("Repeat dummy child: {}", dummy.borrow().short_id());
            FSMNode::add_child_to_all_leaves(&subroot, &dummy);
            // must have the option to skip it entirely
            FSMNode::add_child_cycle_safe(&cur_root, &dummy);

            FSMNode::add_child_to_all_leaves(&subroot, &dummy_parent);
            dummy_parent
        }
        _ => {
            eprintln!("Unimplemented: {cur_node:?}");
            todo!()
        }
    };
    ret
}

fn find_terminal<'a>(grammer: &'a Grammar, name: &'a str) -> Option<&'a Expression> {
    grammer.expressions.iter().find(|expr| expr.lhs == name)
}

/// creates a graph from the provided ebnf
/// Returns an error if the ebnf crate deems the input to be invalid
pub fn create_graph_from_ebnf(ebnf: &str) -> Result<FSMRc<FSMLock<FSMNode>>, String> {
    match ebnf::get_grammar(ebnf) {
        Ok(mut grammar) => {
            let root = FSMNode::new_null(None);
            let root_node = grammar.expressions.get(0).ok_or("Empty BNF!")?.to_owned();
            let mut terminals = HashMap::with_capacity(grammar.expressions.len());
            handle_node(
                &mut grammar,
                Node::Terminal(root_node.lhs.to_owned()),
                &root,
                &mut terminals,
            );
            // sanity op, is_done() won't cancel preemptively
            FSMNode::add_child_to_all_leaves(&root, &FSMNode::new_null(None));
            FSMNode::minify(&root);
            // UserdefCombos will want to know their Keyword children's first character to
            // properly be able to finish
            FSMNode::set_userdef_links(&root);
            debug_println!("Total node cnt: {}", root.borrow().node_cnt());
            // for (name, term) in terminals.iter() {
            //     println!("Term {}", name);
            //     term.0.borrow().dbg();
            // }
            Ok(root)
        }
        Err(err) => Err(err.to_string()),
    }
}

extern crate test;

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    use super::*;
    use test::Bencher;
    #[bench]
    fn benchmark(b: &mut Bencher) {
        let ebnf = read_to_string("../js.ebnf").unwrap();
        b.iter(|| create_graph_from_ebnf(&ebnf));
    }
}
