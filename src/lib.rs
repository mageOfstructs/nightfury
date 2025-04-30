use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

struct RootNode {
    children: Vec<TreeNode>,
}

#[derive(Debug)]
pub struct TreeNode {
    expanded_name: String,
    short_name: String,
    parent: Option<Rc<RefCell<TreeNode>>>,
    children: Vec<Rc<RefCell<TreeNode>>>,
}

impl TreeNode {
    pub fn new(expanded_name: String, short_name: String) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            expanded_name,
            short_name,
            parent: None,
            children: Vec::new(),
        }))
    }

    pub fn dbg(&self) {
        for child in self.children.iter() {
            print!("{} ", child.borrow().expanded_name);
        }
        println!();
        for child in self.children.iter() {
            child.borrow().dbg();
        }
    }

    pub fn new_with_parent(
        expanded_name: String,
        short_name: String,
        parent: Rc<RefCell<TreeNode>>,
    ) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            expanded_name,
            short_name,
            parent: Some(Rc::clone(&parent)),
            children: Vec::new(),
        }));
        parent.borrow_mut().children.push(Rc::clone(&ret));
        ret
    }
}

pub struct TreeCursor {
    cur_ast_pos: Weak<RefCell<TreeNode>>,
    input_buf: String,
}

impl TreeCursor {
    pub fn new(ast_root: &Rc<RefCell<TreeNode>>) -> Self {
        Self {
            cur_ast_pos: Rc::downgrade(ast_root),
            input_buf: String::new(),
        }
    }
    pub fn advance(&mut self, input: char) -> Result<(), String> {
        self.input_buf.push(input);
        // println!("Input buf: {}", self.input_buf);
        let binding = self.cur_ast_pos.upgrade().expect("Tree failure");
        let borrow = binding.borrow();
        let possibly_next_node = borrow
            .children
            .iter()
            .find(|child| child.borrow().short_name == self.input_buf);
        if let Some(next_node) = possibly_next_node {
            // println!("Found: {}", next_node.borrow().expanded_name);
            let next_node = Rc::clone(&next_node);
            self.cur_ast_pos = Rc::downgrade(&Rc::clone(&next_node));
            self.input_buf.clear();
        }
        Ok(())
    }
    fn dump(&self) {
        println!(
            "Last matched node: {}",
            self.cur_ast_pos.upgrade().unwrap().borrow().expanded_name
        );
        println!("Input buf: {}", self.input_buf);
    }
    pub fn get_last_matched_node(&self) -> String {
        self.cur_ast_pos
            .upgrade()
            .unwrap()
            .borrow()
            .expanded_name
            .clone()
    }
    pub fn is_done(&self) -> bool {
        self.cur_ast_pos
            .upgrade()
            .unwrap()
            .borrow()
            .children
            .is_empty()
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
        let root = TreeNode::new("int".to_string(), "i".to_string());
        let _other = TreeNode::new_with_parent("asdf".to_string(), "a".to_string(), root.clone());
        assert_eq!(root.borrow().children.len(), 1);
    }

    #[test]
    fn simple_cursor_steps() {
        let root = TreeNode::new("BEGIN".to_string(), String::new());
        let second = TreeNode::new_with_parent("int".to_string(), "i".to_string(), root.clone());
        TreeNode::new_with_parent("asdf".to_string(), "a".to_string(), second.clone());
        let mut cursor = TreeCursor {
            cur_ast_pos: Rc::downgrade(&root),
            input_buf: String::new(),
        };
        assert_eq!(cursor.get_last_matched_node(), "BEGIN");
        cursor.advance('i').unwrap();
        assert_eq!(cursor.get_last_matched_node(), "int");
        cursor.advance('a').unwrap();
        assert_eq!(cursor.get_last_matched_node(), "asdf");
    }
}
