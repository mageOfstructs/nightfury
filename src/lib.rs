#![feature(let_chains)]

use std::cell::RefCell;
use std::rc::{Rc, Weak};

// ironic that this only expands names
struct NameShortener;
impl NameShortener {
    fn expand(old: Option<&str>, full: &str) -> String {
        if full.is_empty() {
            panic!("Cannot expand the void!")
        }
        let ret = if let Some(old) = old {
            if full.len() < old.len() {
                panic!("NS: There is nothing left...")
            }
            let mut ret = old.to_string();
            ret.push_str(&full[old.len()..old.len() + 1]);
            ret
        } else {
            full[0..1].to_string()
        };
        println!("Got {} instead of {old:?}", ret);
        ret
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyword {
    short: String,
    expanded: String,
    closing_token: Option<String>,
}

impl Keyword {
    pub fn new(expanded: String, closing_token: Option<String>) -> Self {
        Self {
            short: expanded.chars().nth(0).unwrap().to_string(),
            expanded,
            closing_token,
        }
    }
}

impl Default for Keyword {
    fn default() -> Self {
        Self {
            short: String::new(),
            expanded: String::new(),
            closing_token: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Keyword(Keyword),
    UserDefined { final_chars: Vec<char> },
    Null,
}

use NodeType::*;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeValue {
    pub ntype: NodeType,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    value: NodeType,
    parent: Option<Rc<RefCell<TreeNode>>>,
    children: Vec<Rc<RefCell<TreeNode>>>,
}

impl TreeNode {
    pub fn add_child(&mut self, child: &Rc<RefCell<TreeNode>>) {
        while self.handle_potential_conflict(child) {}
        self.children.push(Rc::clone(&child));
    }
    pub fn new_null(parent: Option<&Rc<RefCell<TreeNode>>>) -> Rc<RefCell<Self>> {
        let parent_ref = if let Some(parent) = parent {
            Some(Rc::clone(parent))
        } else {
            None
        };
        let ret = Rc::new(RefCell::new(Self {
            value: Null,
            parent: parent_ref,
            children: Vec::new(),
        }));
        if let Some(parent) = parent {
            parent.borrow_mut().children.push(Rc::clone(&ret));
        }
        ret
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

    pub fn new(value: NodeType, parent: &Rc<RefCell<TreeNode>>) -> Rc<RefCell<Self>> {
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
            value,
            parent: Some(Rc::clone(parent)),
            children: Vec::new(),
        }));
        parent.borrow_mut().children.push(Rc::clone(&ret));
        ret
    }

    pub fn new_keyword(expanded_name: String) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            value: Keyword(Keyword {
                short: expanded_name.chars().nth(0).unwrap().to_string(),
                expanded: expanded_name,
                ..Default::default()
            }),
            parent: None,
            children: Vec::new(),
        }))
    }

    pub fn new_keyword_with_parent(
        expanded_name: String,
        parent: Rc<RefCell<TreeNode>>,
    ) -> Rc<RefCell<Self>> {
        let ret = Self::new_keyword(expanded_name);
        ret.borrow_mut().parent = Some(Rc::clone(&parent));
        parent.borrow_mut().add_child(&ret);
        ret
    }
    fn find_node_with_code(&self, short: &str) -> Option<Rc<RefCell<TreeNode>>> {
        for child in &self.children {
            if let Keyword(Keyword { short: nshort, .. }) = &child.borrow().value
                && nshort == short
            {
                return Some(Rc::clone(&child));
            }
        }
        for child in &self.children {
            let rec_res = child.borrow().find_node_with_code(short);
            if rec_res.is_some() {
                return rec_res;
            }
        }
        None
    }

    fn check_for_conflicts(&self, short: &str) -> bool {
        for child in &self.children {
            let borrow = child.borrow();
            match &borrow.value {
                Keyword(Keyword { short: nshort, .. }) if nshort == short => return true,
                Null => {
                    let rec_res = borrow.check_for_conflicts(short);
                    if rec_res {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn get_conflicting_node(&self, short: &str) -> Option<Rc<RefCell<TreeNode>>> {
        for child in &self.children {
            let borrow = child.borrow();
            match &borrow.value {
                Keyword(Keyword { short: nshort, .. }) if short.starts_with(nshort) => {
                    return Some(Rc::clone(&child));
                }
                Null => {
                    let rec_res = borrow.get_conflicting_node(short);
                    if rec_res.is_some() {
                        return rec_res;
                    }
                }
                _ => {}
            }
        }
        None
    }
    fn handle_potential_conflict_internal(&mut self, child: &Rc<RefCell<TreeNode>>) -> bool {
        let child_borrow = child.borrow();
        let mut ret = false;
        if let Keyword(Keyword { short: cshort, .. }) = &child_borrow.value {
            if let Some(node) = self.get_conflicting_node(cshort)
                && node.borrow().value != child_borrow.value
            {
                node.replace_with(|node| {
                    if let Keyword(keyword_struct) = &mut node.value {
                        let new_short = NameShortener::expand(
                            Some(&keyword_struct.short),
                            &keyword_struct.expanded,
                        );
                        keyword_struct.short = new_short;
                        println!("conflict handler 2");
                        ret = true;
                        node.to_owned()
                    } else {
                        panic!(
                            "What?! We got a non-keyword node from the get_conflicting_node fn! Anyways, I'm gonna snuggle some foxxos now..."
                        )
                    }
                });
            }
        }
        ret
    }
    fn handle_potential_conflict(&mut self, child: &Rc<RefCell<TreeNode>>) -> bool {
        let child_borrow = child.borrow();
        if let Keyword(keyword_struct) = &child_borrow.value {
            println!("{:?}", self.value);
            println!("{:?}", child.borrow().value);
            if self.handle_potential_conflict_internal(child) {
                let short =
                    NameShortener::expand(Some(&keyword_struct.short), &keyword_struct.expanded);
                drop(child_borrow);
                child.replace_with(|node| {
                    if let Keyword(k) = &mut node.value {
                        k.short = short;
                    } else {
                        unreachable!()
                    }
                    node.to_owned()
                });
                return true;
            }
        } else if let Null = &child_borrow.value {
            let mut ret = false;
            child_borrow.children.iter().for_each(|child| {
                if self.handle_potential_conflict_internal(child) {
                    let mut mut_child = child.borrow_mut();
                    if let Keyword(k) = &mut mut_child.value {
                        k.short = NameShortener::expand(Some(&k.short), &k.expanded);
                    }
                    ret = true;
                }
            });
            if ret {
                return true;
            }
        }
        false
    }
    pub fn dump_children(&self) {
        self.children
            .iter()
            .for_each(|child| println!("{:?}", child.borrow().value));
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
            let ret = if let NodeType::Keyword(Keyword {
                short,
                expanded,
                closing_token: None,
                ..
            }) = &next_node.borrow().value
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
    pub fn search_rec(
        &self,
        treenode: &Rc<RefCell<TreeNode>>,
        potential_matches: &mut u32,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        if *potential_matches > 1 {
            return None; // don't even try
        }
        // println!("search_rec: {:?}", treenode.borrow().value);
        // println!("{}\n", self.input_buf);
        let binding = treenode;
        let borrow = binding.borrow();
        let mut keyword_match = None;
        for child in &borrow.children {
            let node_val = &child.borrow().value;
            match node_val {
                NodeType::Keyword(Keyword { short, .. }) if short.starts_with(&self.input_buf) => {
                    println!("{:?}", child.borrow().value);
                    println!("{short} == {}", self.input_buf);
                    keyword_match = Some(child.clone());
                    *potential_matches += 1;
                    if *potential_matches > 1 {
                        break;
                    }
                }
                Null => {
                    println!("RecParent: {:?}", child.borrow().value);
                    let rec_res = self.search_rec(&child, potential_matches);
                    if rec_res.is_some() {
                        println!("Recursive: {:?}", rec_res.as_ref().unwrap().borrow().value);
                        // *potential_matches += 1;
                        keyword_match = rec_res;
                        if *potential_matches > 1 {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        println!("pm: {potential_matches}");
        if keyword_match.is_some() && *potential_matches == 1 {
            return keyword_match;
        }

        // so we can start typing right away
        let userdef_match = borrow.children.iter().find(|child| {
            if let NodeType::UserDefined { .. } = child.borrow().value {
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
            NodeType::UserDefined { final_chars, .. } => {
                let res = self.handle_userdefined(input, final_chars);
                if res.is_some() {
                    return res;
                }
            }
            _ => {
                let res = self.search_rec(&binding, &mut 0);
                if let Some(node) = res {
                    self.update_cursor(&node);
                    return match &node.borrow().value {
                        NodeType::Keyword(Keyword { expanded, .. }) => {
                            self.input_buf.clear();
                            Some(expanded.clone())
                        }
                        NodeType::UserDefined { final_chars } => {
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
        if let NodeType::Keyword(Keyword {
            closing_token: Some(_),
            ..
        }) = &node.borrow().value
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
        if let NodeType::UserDefined { .. } = self.get_cur_ast_binding().borrow().value {
            true
        } else {
            false
        }
    }

    fn get_current_nodeval(&self) -> NodeType {
        self.get_cur_ast_binding().borrow().value.clone()
    }
    fn find_node_with_code(&self, short: &str) -> Option<Rc<RefCell<TreeNode>>> {
        let binding = self.get_cur_ast_binding();
        let binding = binding.borrow();
        binding.find_node_with_code(short)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_tree() {
        let root = TreeNode::new_keyword("int".to_string());
        let _other = TreeNode::new_keyword_with_parent("asdf".to_string(), root.clone());
        assert_eq!(root.borrow().children.len(), 1);
    }

    #[test]
    fn simple_cursor_steps() {
        let root = TreeNode::new_null(None);
        let second = TreeNode::new_keyword_with_parent("int".to_string(), root.clone());
        TreeNode::new_keyword_with_parent("asdf".to_string(), second.clone());
        let mut cursor = TreeCursor::new(&root);
        assert_eq!(cursor.get_current_nodeval(), Null);
        cursor.advance('i').unwrap();
        assert_eq!(
            cursor.get_current_nodeval(),
            NodeType::Keyword(Keyword::new("int".to_string(), None)),
        );
        cursor.advance('a').unwrap();
        assert_eq!(
            cursor.get_current_nodeval(),
            NodeType::Keyword(Keyword::new("asdf".to_string(), None)),
        );
    }

    #[test]
    fn test_conflict_check() {
        let root = TreeNode::new_null(None);
        let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
        let child = TreeNode::new(sign_token.clone(), &root);
        sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

        let child2 = TreeNode::new(sign_token, &root);
        let types = TreeNode::new_required(NodeType::Null, &child);

        let int = TreeNode::new_keyword_with_parent("int".to_string(), types.clone());
        let float = TreeNode::new_keyword_with_parent("short".to_string(), types.clone());
        child.borrow_mut().add_child(&types);
        child2.borrow_mut().add_child(&types);

        assert!(root.borrow().check_for_conflicts("s"));
        assert!(child2.borrow().check_for_conflicts("s"));
        assert!(types.borrow().check_for_conflicts("s"));
        assert!(!int.borrow().check_for_conflicts("s"));
    }

    #[test]
    fn test_keyword_matching() {
        let root = TreeNode::new_null(None);
        let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
        let child = TreeNode::new(sign_token.clone(), &root);
        sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

        let child2 = TreeNode::new(sign_token, &root);
        let types = TreeNode::new_required(NodeType::Null, &child);

        let int = TreeNode::new_keyword_with_parent("int".to_string(), types.clone());
        let float = TreeNode::new_keyword_with_parent("short".to_string(), types.clone());
        root.borrow_mut().add_child(&types);
        child2.borrow_mut().add_child(&types);

        let mut cursor = TreeCursor::new(&root);
        assert!(cursor.advance('s').is_none());
        assert!(cursor.advance('h').is_some());
        let mut cursor = TreeCursor::new(&root);
        assert!(cursor.advance('u').is_some());
        assert!(cursor.advance('s').is_some());
    }
}
