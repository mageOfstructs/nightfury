#![feature(let_chains)]

use core::borrow;
use std::cell::Ref;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

struct RootNode {
    children: Vec<TreeNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Keyword {
        short: String,
        expanded: String,
        closing_token: Option<String>,
    },
    UserDefined {
        final_chars: Vec<char>,
    },
    Null,
}

use NodeType::*;

impl NodeType {
    fn get_keyword_default() -> Self {
        Self::Keyword {
            short: String::new(),
            expanded: String::new(),
            closing_token: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeValue {
    pub ntype: NodeType,
    pub optional: bool,
}

#[derive(Debug)]
pub struct TreeNode {
    value: NodeValue,
    parent: Option<Rc<RefCell<TreeNode>>>,
    children: Vec<Rc<RefCell<TreeNode>>>,
}

impl TreeNode {
    pub fn add_child(&mut self, child: &Rc<RefCell<TreeNode>>) {
        self.children.push(Rc::clone(&child));
    }
    pub fn new_keyword(expanded_name: String, short_name: String) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            value: NodeValue {
                ntype: Keyword {
                    short: short_name,
                    expanded: expanded_name,
                    closing_token: None,
                },
                optional: false,
            },
            parent: None,
            children: Vec::new(),
        }))
    }

    pub fn dbg(&self) {
        for child in self.children.iter() {
            print!("{:?} ", child.borrow().value);
        }
        println!();
        for child in self.children.iter() {
            child.borrow().dbg();
        }
    }

    pub fn new(value: NodeValue, parent: &Rc<RefCell<TreeNode>>) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value,
            parent: Some(Rc::clone(parent)),
            children: Vec::new(),
        }));
        parent.borrow_mut().children.push(Rc::clone(&ret));
        ret
    }

    pub fn new_required(value: NodeType, parent: &Rc<RefCell<TreeNode>>) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value: NodeValue {
                ntype: value,
                optional: false,
            },
            parent: Some(Rc::clone(parent)),
            children: Vec::new(),
        }));
        parent.borrow_mut().children.push(Rc::clone(&ret));
        ret
    }
    pub fn new_keyword_with_parent(
        expanded_name: String,
        short_name: String,
        parent: Rc<RefCell<TreeNode>>,
    ) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value: NodeValue {
                ntype: NodeType::Keyword {
                    short: short_name,
                    expanded: expanded_name,
                    closing_token: None,
                },
                optional: false,
            },
            parent: Some(Rc::clone(&parent)),
            children: Vec::new(),
        }));
        parent.borrow_mut().children.push(Rc::clone(&ret));
        ret
    }
}

type InternalCursor = Weak<RefCell<TreeNode>>;
pub struct TreeCursor {
    cur_ast_pos: InternalCursor,
    input_buf: String,
    unfinished_nodes: Vec<InternalCursor>,
}

impl TreeCursor {
    pub fn new(ast_root: &Rc<RefCell<TreeNode>>) -> Self {
        Self {
            cur_ast_pos: Rc::downgrade(ast_root),
            input_buf: String::new(),
            unfinished_nodes: Vec::new(),
        }
    }
    fn handle_userdefined(&mut self, input: char, final_chars: &Vec<char>) -> Option<String> {
        let child_idx = final_chars.iter().position(|char| *char == input);
        if let Some(child_idx) = child_idx {
            let strong_ref = self.get_cur_ast_binding();
            let borrow = strong_ref.borrow();
            let next_node = Rc::clone(&borrow.children[child_idx]);
            self.update_cursor(&next_node);
            let ret = if let NodeValue {
                ntype:
                    NodeType::Keyword {
                        short,
                        expanded,
                        closing_token: None,
                    },
                ..
            } = &next_node.borrow().value
                && *short == String::from(input)
            {
                Some(expanded.clone())
            } else {
                Some(String::from(final_chars[child_idx]))
            };
            self.input_buf.clear();
            ret
        } else {
            None
        }
    }
    pub fn clear_inputbuf(&mut self) {
        self.input_buf.clear();
    }
    pub fn search_rec(&self, treenode: &Rc<RefCell<TreeNode>>) -> Option<Rc<RefCell<TreeNode>>> {
        // println!("search_rec: {:?}", treenode.borrow().value);
        // println!("{}\n", self.input_buf);
        let binding = treenode;
        let borrow = binding.borrow();
        let mut keyword_match = None;
        for child in &borrow.children {
            let node_val = &child.borrow().value;
            match node_val {
                NodeValue {
                    ntype: NodeType::Keyword { short, .. },
                    ..
                } if self.input_buf == *short => {
                    keyword_match = Some(child.clone());
                    break;
                }
                NodeValue {
                    ntype: NodeType::Null,
                    ..
                }
                | NodeValue { optional: true, .. } => {
                    keyword_match = self.search_rec(&child);
                    if keyword_match.is_some() {
                        break;
                    }
                }
                _ => {}
            }
        }
        if keyword_match.is_some() {
            return keyword_match;
        }

        // so we can start typing right away
        let userdef_match = borrow.children.iter().find(|child| {
            if let NodeValue {
                ntype: NodeType::UserDefined { .. },
                ..
            } = child.borrow().value
            {
                true
            } else {
                false
            }
        });
        if userdef_match.is_some() {
            return userdef_match.cloned();
        }
        None
    }
    pub fn advance(&mut self, input: char) -> Option<String> {
        let binding = self.cur_ast_pos.upgrade().expect("Tree failure");
        let borrow = binding.borrow();
        self.input_buf.push(input);
        match &borrow.value {
            // NodeType::Null | NodeType::Keyword { .. } => {}
            // NodeType::Keyword { expanded, .. } => {
            //     self.input_buf.push(input);
            //     let possibly_next_node = borrow.children.iter().find(|child| {
            //         if let NodeType::Keyword { short, .. } = &child.borrow().value {
            //             *short == self.input_buf
            //         } else {
            //             false
            //         }
            //     });
            //     if let Some(NodeType::Keyword { expanded, .. }) =
            //         possibly_next_node.and_then(|res| Some(res.borrow().value.clone()))
            //     {
            //         let next_node = Rc::clone(&possibly_next_node.unwrap());
            //         println!("{:?}", next_node.borrow().value);
            //         self.cur_ast_pos = Rc::downgrade(&Rc::clone(&next_node));
            //         self.input_buf.clear();
            //         return Some(expanded);
            //     }
            // }
            NodeValue {
                ntype: NodeType::UserDefined { final_chars, .. },
                ..
            } => {
                let res = self.handle_userdefined(input, final_chars);
                if res.is_some() {
                    return res;
                }
            }
            _ => {
                let res = self.search_rec(&binding);
                if let Some(node) = res {
                    self.update_cursor(&node);
                    return match &node.borrow().value {
                        NodeValue {
                            ntype: NodeType::Keyword { expanded, .. },
                            ..
                        } => {
                            self.input_buf.clear();
                            Some(expanded.clone())
                        }
                        NodeValue {
                            ntype: NodeType::UserDefined { final_chars },
                            ..
                        } => {
                            let res = self.handle_userdefined(input, &final_chars);
                            res
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
        None
    }

    fn update_cursor(&mut self, node: &Rc<RefCell<TreeNode>>) {
        self.cur_ast_pos = Rc::downgrade(&Rc::clone(&node));
        if let NodeValue {
            ntype:
                NodeType::Keyword {
                    closing_token: Some(_),
                    ..
                },
            ..
        } = &node.borrow().value
        {
            self.unfinished_nodes.push(Rc::downgrade(&node));
        } else if node.borrow().children.is_empty() && self.unfinished_nodes.len() > 1 {
            // we don't need to jump back if only one remains
            self.cur_ast_pos = self.unfinished_nodes.pop().unwrap();
        }
        println!("{:?}", self.get_cur_ast_binding().borrow().value);
    }
    fn dump(&self) {
        println!(
            "Last matched node: {:?}",
            self.cur_ast_pos.upgrade().unwrap().borrow().value
        );
        println!("Input buf: {}", self.input_buf);
    }

    pub fn is_done(&self) -> bool {
        let ast_ref = self.cur_ast_pos.upgrade().unwrap();
        let binding = ast_ref.borrow();
        binding.children.is_empty()
    }

    fn get_cur_ast_binding(&self) -> Rc<RefCell<TreeNode>> {
        self.cur_ast_pos.upgrade().unwrap()
    }
    pub fn is_in_userdefined_stage(&self) -> bool {
        if let NodeValue {
            ntype: NodeType::UserDefined { .. },
            ..
        } = self.get_cur_ast_binding().borrow().value
        {
            true
        } else {
            false
        }
    }

    fn get_current_nodeval(&self) -> NodeValue {
        self.get_cur_ast_binding().borrow().value.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn simple_tree() {
        let root = TreeNode::new_keyword("int".to_string(), "i".to_string());
        let _other =
            TreeNode::new_keyword_with_parent("asdf".to_string(), "a".to_string(), root.clone());
        assert_eq!(root.borrow().children.len(), 1);
    }

    #[test]
    fn simple_cursor_steps() {
        let root = TreeNode::new_keyword("BEGIN".to_string(), String::new());
        let second =
            TreeNode::new_keyword_with_parent("int".to_string(), "i".to_string(), root.clone());
        TreeNode::new_keyword_with_parent("asdf".to_string(), "a".to_string(), second.clone());
        let mut cursor = TreeCursor::new(&root);
        assert_eq!(cursor.get_current_nodeval(), NodeValue {
            ntype: NodeType::Keyword {
                short: String::new(),
                expanded: String::from("BEGIN"),
                closing_token: None
            },
            optional: false
        });
        cursor.advance('i').unwrap();
        assert_eq!(cursor.get_current_nodeval(), NodeValue {
            ntype: NodeType::Keyword {
                short: String::from("i"),
                expanded: String::from("int"),
                closing_token: None
            },
            optional: false
        });
        cursor.advance('a').unwrap();
        assert_eq!(cursor.get_current_nodeval(), NodeValue {
            ntype: NodeType::Keyword {
                short: String::from("a"),
                expanded: String::from("asdf"),
                closing_token: None
            },
            optional: false
        });
    }
}
