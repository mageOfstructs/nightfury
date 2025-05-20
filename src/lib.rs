#![feature(let_chains)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

pub mod frontend;

// ironic that this only expands names
struct NameShortener;
impl NameShortener {
    fn expand(old: Option<&str>, full: &str) -> String {
        if full.is_empty() {
            panic!("Cannot expand the void!")
        }
        let ret = if let Some(old) = old {
            if old == full {
                return old.to_string(); // FIXME: this can't be a good handling
            }
            if full.len() < old.len() {
                panic!("NS: There is nothing left...")
            }
            let mut ret = old.to_string();
            ret.push_str(&full[old.len()..old.len() + 1]);
            ret
        } else {
            full[0..1].to_string()
        };
        debug_println!("Got {} instead of {old:?}", ret);
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

// TODO: combine UserDefined and UserDefinedRegex into one variant
#[derive(Debug, Clone)]
pub enum NodeType {
    Keyword(Keyword),
    UserDefined { final_chars: Vec<char> },
    UserDefinedRegex(Regex),
    Null,
}

impl PartialEq for NodeType {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Keyword(k) => match other {
                Keyword(k2) => k.eq(k2),
                _ => false,
            },
            UserDefined { final_chars: fc } => match other {
                UserDefined { final_chars: fc2 } => fc.eq(fc2),
                _ => false,
            },
            UserDefinedRegex(r) => match other {
                UserDefinedRegex(r2) => r.as_str().eq(r2.as_str()),
                _ => false,
            },
            Null => match other {
                Null => true,
                _ => false,
            },
        }
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

trait PartialMatch {
    fn partial_match(&self, hay: &str) -> bool;
}

impl PartialMatch for Regex {
    /**
    not a perfect solution and should therefore be never used
    **/
    fn partial_match(&self, hay: &str) -> bool {
        let orig = self.as_str();
        for i in 1..orig.len() {
            if let Ok(regex) = Regex::new(&orig[0..=i])
                && regex.is_match(hay)
            {
                return true;
            }
        }
        false
    }
}

use NodeType::*;
use debug_print::debug_println;
use regex::Regex;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeValue {
    pub ntype: NodeType,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    id: Uuid,
    value: NodeType,
    parent: Option<Rc<RefCell<TreeNode>>>,
    children: Vec<Rc<RefCell<TreeNode>>>,
}

impl Default for TreeNode {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            value: Null,
            parent: None,
            children: Vec::new(),
        }
    }
}

impl TreeNode {
    fn deep_clone_internal(
        stub: &Rc<RefCell<Self>>,
        old: &TreeNode,
        visited_nodes: &mut HashMap<Uuid, Rc<RefCell<TreeNode>>>,
    ) -> Rc<RefCell<Self>> {
        for child in &old.children {
            if !visited_nodes.contains_key(&child.borrow().id) {
                let clone = Rc::new(RefCell::new(Self {
                    id: Uuid::new_v4(),
                    value: child.borrow().value.clone(),
                    parent: None, // is deprecated anyways
                    children: Vec::new(),
                }));
                visited_nodes.insert(child.borrow().id, clone.clone());
                TreeNode::deep_clone_internal(&clone, &child.borrow(), visited_nodes);
                stub.borrow_mut().children.push(clone);
            } else {
                stub.borrow_mut()
                    .children
                    .push(visited_nodes.get(&child.borrow().id).unwrap().clone());
            }
        }
        stub.clone()
    }
    // TODO: make deep_clone private
    pub fn deep_clone(&self) -> Rc<RefCell<Self>> {
        debug_println!("Deep cloning node {}", self.short_id());
        let ret = Rc::new(RefCell::new(Self {
            id: Uuid::new_v4(),
            value: self.value.clone(),
            parent: None, // is deprecated anyways
            children: Vec::new(),
        }));
        let mut visited_nodes = HashMap::new();
        visited_nodes.insert(self.id, ret.clone());
        let ret = TreeNode::deep_clone_internal(&ret, self, &mut visited_nodes);
        debug_println!("Finish deep clone:");
        ret.borrow().dbg();
        ret
    }
    fn do_stuff_cycle_aware(
        &self,
        op: &mut impl FnMut(&TreeNode, Rc<RefCell<TreeNode>>) -> bool,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        self.do_stuff_cycle_aware_internal(op, &mut HashSet::new())
    }
    fn do_stuff_cycle_aware_internal(
        &self,
        op: &mut impl FnMut(&TreeNode, Rc<RefCell<TreeNode>>) -> bool,
        visited_nodes: &mut HashSet<Uuid>,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        for child in &self.children {
            if !visited_nodes.contains(&child.borrow().id) {
                visited_nodes.insert(child.borrow().id);
                if op(self, child.clone()) {
                    return Some(child.clone());
                }
                if let Some(child) = child
                    .borrow()
                    .do_stuff_cycle_aware_internal(op, visited_nodes)
                {
                    return Some(child);
                }
            }
        }
        None
    }

    fn do_stuff_cycle_aware_non_greedy(
        &self,
        op: &mut impl FnMut(Rc<RefCell<TreeNode>>) -> bool,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        self.do_stuff_cycle_aware_non_greedy_internal(op, &mut HashSet::new())
    }
    fn do_stuff_cycle_aware_non_greedy_internal(
        &self,
        op: &mut impl FnMut(Rc<RefCell<TreeNode>>) -> bool,
        visited_nodes: &mut HashSet<Uuid>,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        for child in &self.children {
            if !visited_nodes.contains(&child.borrow().id) {
                visited_nodes.insert(child.borrow().id);
                if op(child.clone()) {
                    return Some(child.clone());
                }
                if let Null = child.borrow().value
                    && let Some(ret) = child
                        .borrow()
                        .do_stuff_cycle_aware_non_greedy_internal(op, visited_nodes)
                {
                    return Some(ret);
                }
            }
        }
        None
    }
    fn has_useful_children(&self) -> bool {
        self.do_stuff_cycle_aware(&mut |_, c| match c.borrow().value {
            Null => false,
            _ => true,
        })
        .is_some()
    }

    pub fn get_last_child(&self) -> Option<Rc<RefCell<TreeNode>>> {
        self.children.last().cloned()
    }
    pub fn add_child(&mut self, child: &Rc<RefCell<TreeNode>>) {
        while self.handle_potential_conflict(child) {}
        self.children.push(Rc::clone(&child));
    }
    pub fn add_child_cycle_safe(this: &Rc<RefCell<TreeNode>>, child: &Rc<RefCell<TreeNode>>) {
        while this.borrow().handle_potential_conflict(child) {}
        this.borrow_mut().children.push(Rc::clone(&child));
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
            ..Default::default()
        }));
        if let Some(parent) = parent {
            parent.borrow_mut().add_child(&ret);
        }
        ret
    }

    fn short_id(&self) -> String {
        self.id.simple().to_string()[0..6].to_string()
    }

    fn dbg_internal(&self, indent: usize, visited_nodes: &mut HashSet<Uuid>) {
        println!("{}{:?} {}", " ".repeat(indent), self.value, self.short_id());
        visited_nodes.insert(self.id);
        for child in self.children.iter() {
            if !visited_nodes.contains(&child.borrow().id) {
                child.borrow().dbg_internal(indent + 4, visited_nodes);
            } else {
                println!("{}Cycle to {}", " ".repeat(indent + 4), child.borrow().id);
            }
        }
    }

    fn get_all_leaves_internal(
        &self,
        discovered_leaves: &mut Vec<Rc<RefCell<TreeNode>>>,
        visited_nodes: &mut HashSet<Uuid>,
    ) {
        for child in &self.children {
            // debug_println!("at node {:?}; {}", child.borrow().value, child.borrow().id);
            if child.borrow().children.is_empty() {
                // debug_println!("adding node {:?}", child.borrow().value);
                discovered_leaves.push(child.clone());
            } else if !visited_nodes.contains(&child.borrow().id) {
                visited_nodes.insert(child.borrow().id);
                child
                    .borrow()
                    .get_all_leaves_internal(discovered_leaves, visited_nodes);
            }
        }
    }
    fn get_all_leaves(&self, discovered_leaves: &mut Vec<Rc<RefCell<TreeNode>>>) {
        self.get_all_leaves_internal(discovered_leaves, &mut HashSet::new());
    }
    pub fn add_child_to_all_leaves(this: &Rc<RefCell<TreeNode>>, child: &Rc<RefCell<TreeNode>>) {
        let mut leaves = Vec::new();
        this.borrow().get_all_leaves(&mut leaves);
        while let Some(node) = leaves.pop() {
            if node.borrow().children.is_empty() {
                TreeNode::add_child_cycle_safe(&node, child);
            }
        }
        if this.borrow().children.is_empty() {
            TreeNode::add_child_cycle_safe(&this, child);
        }
    }

    pub fn race_to_leaf(&self) -> Option<Rc<RefCell<TreeNode>>> {
        self.do_stuff_cycle_aware(&mut |_, child| child.borrow().children.is_empty())
    }
    pub fn dbg(&self) {
        self.dbg_internal(0, &mut HashSet::new());
    }

    pub fn new(value: NodeType, parent: &Rc<RefCell<TreeNode>>) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value,
            parent: Some(Rc::clone(parent)),
            children: Vec::new(),
            ..Default::default()
        }));
        parent.borrow_mut().add_child(&ret);
        ret
    }

    pub fn new_required(value: NodeType, parent: &Rc<RefCell<TreeNode>>) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value,
            parent: Some(Rc::clone(parent)),
            children: Vec::new(),
            ..Default::default()
        }));
        parent.borrow_mut().add_child(&ret);
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
            ..Default::default()
        }))
    }

    pub fn new_keyword_with_parent(
        expanded_name: String,
        parent: Rc<RefCell<TreeNode>>,
    ) -> Rc<RefCell<Self>> {
        let ret = Self::new_keyword(expanded_name);
        ret.borrow_mut().parent = Some(Rc::clone(&parent));
        TreeNode::add_child_cycle_safe(&parent, &ret);
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
        self.do_stuff_cycle_aware_non_greedy(&mut |child: Rc<RefCell<TreeNode>>| {
            println!("awa?");
            match &child.borrow().value {
                Keyword(Keyword { short: nshort, .. }) if short.starts_with(nshort) => {
                    return true;
                }
                _ => false,
            }
        })
    }
    fn handle_potential_conflict_internal(&self, child: &Rc<RefCell<TreeNode>>) -> bool {
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
                        debug_println!("conflict handler 2");
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
    pub fn handle_potential_conflict(&self, child: &Rc<RefCell<TreeNode>>) -> bool {
        let child_borrow = child.borrow();
        if let Keyword(keyword_struct) = &child_borrow.value {
            debug_println!("{:?}", self.value);
            debug_println!("{:?}", child.borrow().value);
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
            // println!("awa?");
            let mut ret = false;
            self.do_stuff_cycle_aware(&mut |_, child| {
                if self.handle_potential_conflict_internal(&child) {
                    let mut mut_child = child.borrow_mut();
                    if let Keyword(k) = &mut mut_child.value {
                        k.short = NameShortener::expand(Some(&k.short), &k.expanded);
                    }
                    ret = true;
                }
                false
            });
            // child_borrow.children.iter().for_each(|child| {
            //     if self.handle_potential_conflict_internal(child) {
            //         let mut mut_child = child.borrow_mut();
            //         if let Keyword(k) = &mut mut_child.value {
            //             k.short = NameShortener::expand(Some(&k.short), &k.expanded);
            //         }
            //         ret = true;
            //     }
            // });
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
    fn search_for_userdefs(
        &self,
        treenode: &Rc<RefCell<TreeNode>>,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        treenode
            .borrow()
            .do_stuff_cycle_aware_non_greedy(&mut |child| match child.borrow().value {
                UserDefined { .. } | UserDefinedRegex { .. } => true,
                _ => false,
            })
        // for child in &treenode.borrow().children {
        //     match child.borrow().value {
        //         UserDefined { .. } | UserDefinedRegex { .. } => return Some(child.clone()),
        //         Null => {
        //             let rec_res = self.search_for_userdefs(&child);
        //             if rec_res.is_some() {
        //                 return rec_res;
        //             }
        //         }
        //         _ => {}
        //     }
        // }
        // None
    }
    pub fn search_rec(
        &self,
        treenode: &Rc<RefCell<TreeNode>>,
        potential_matches: &mut u32,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        // if *potential_matches > 1 {
        //     return None; // don't even try
        // }
        // println!("search_rec: {:?}", treenode.borrow().value);
        // println!("{}\n", self.input_buf);
        debug_println!(
            "search_rec at {:?} {}",
            treenode.borrow().value,
            treenode.borrow().short_id()
        );
        let binding = treenode;
        let borrow = binding.borrow();
        let mut keyword_match = None;
        treenode
            .borrow()
            .do_stuff_cycle_aware_non_greedy(&mut |child| {
                let node_val = &child.borrow().value;
                match node_val {
                    NodeType::Keyword(Keyword { short, .. })
                        if short.starts_with(&self.input_buf) =>
                    {
                        debug_println!("{:?}", child.borrow().value);
                        debug_println!("{short} == {}", self.input_buf);
                        keyword_match = Some(child.clone());
                        *potential_matches += 1;
                        *potential_matches > 1
                    }
                    // Null => {
                    //     debug_println!("RecParent: {:?}", child.borrow().value);
                    //     let rec_res = self.search_rec(&child, potential_matches);
                    //     if rec_res.is_some() {
                    //         debug_println!(
                    //             "Recursive: {:?}",
                    //             rec_res.as_ref().unwrap().borrow().value
                    //         );
                    //         // *potential_matches += 1;
                    //         keyword_match = rec_res;
                    //         false
                    //     }
                    // }
                    _ => false,
                }
            });
        // for child in &borrow.children {
        //     let node_val = &child.borrow().value;
        //     match node_val {
        //         NodeType::Keyword(Keyword { short, .. }) if short.starts_with(&self.input_buf) => {
        //             debug_println!("{:?}", child.borrow().value);
        //             debug_println!("{short} == {}", self.input_buf);
        //             keyword_match = Some(child.clone());
        //             *potential_matches += 1;
        //             if *potential_matches > 1 {
        //                 break;
        //             }
        //         }
        //         Null => {
        //             debug_println!("RecParent: {:?}", child.borrow().value);
        //             let rec_res = self.search_rec(&child, potential_matches);
        //             if rec_res.is_some() {
        //                 debug_println!("Recursive: {:?}", rec_res.as_ref().unwrap().borrow().value);
        //                 // *potential_matches += 1;
        //                 keyword_match = rec_res;
        //                 if *potential_matches > 1 {
        //                     break;
        //                 }
        //             }
        //         }
        //         _ => {}
        //     }
        // }
        debug_println!("pm: {potential_matches}");
        if keyword_match.is_some() && *potential_matches == 1 {
            return keyword_match;
        }

        // so we can start typing right away
        //
        // let userdef_match = borrow
        //     .children
        //     .iter()
        //     .find(|child| match child.borrow().value {
        //         UserDefined { .. } | UserDefinedRegex(..) => true,
        //         _ => false,
        //     });
        let userdef_match = self.search_for_userdefs(treenode);
        if userdef_match.is_some() {
            return userdef_match;
        }
        None
    }
    pub fn advance(&mut self, input: char) -> Option<String> {
        let binding = self.cur_ast_pos.upgrade().expect("Tree failure");
        let borrow = binding.borrow();
        debug_println!(
            "advance with cursor {:?} {}",
            borrow.value,
            borrow.short_id()
        );
        self.input_buf.push(input);
        match &borrow.value {
            NodeType::UserDefined { final_chars, .. } => {
                let res = self.handle_userdefined(input, final_chars);
                if res.is_some() {
                    return res;
                }
            }
            NodeType::UserDefinedRegex(r) => {
                debug_println!("Checking regex against '{}'", &self.input_buf);
                if r.is_match(&self.input_buf) {
                    let strong_ref = self.get_cur_ast_binding();
                    self.input_buf.clear();
                    self.input_buf.push(input);
                    let mut next_node = self.search_rec(&strong_ref, &mut 0);
                    let borrow = strong_ref.borrow();
                    if next_node.is_none() {
                        next_node =
                            Some(Rc::clone(&borrow.children.get(0).expect(
                                "UserDefinedRegex doesn't have a child and is therefore sad",
                            )));
                    }
                    let next_node = next_node.unwrap();
                    self.update_cursor(&next_node);
                    let ret = if let NodeType::Keyword(Keyword {
                        expanded,
                        closing_token: None,
                        ..
                    }) = &next_node.borrow().value
                    {
                        Some(expanded.clone())
                    } else {
                        Some(input.to_string())
                    };
                    self.input_buf.clear();
                    return ret;
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
                        NodeType::UserDefinedRegex(r) => None,
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
        debug_println!(
            "uc: {:?} {}",
            self.get_cur_ast_binding().borrow().value,
            self.get_cur_ast_binding().borrow().id
        );
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
        match binding.value {
            UserDefinedRegex(..) | UserDefined { .. } => false,
            _ => binding.children.is_empty() || !binding.has_useful_children(),
        }
    }

    fn get_cur_ast_binding(&self) -> Rc<RefCell<TreeNode>> {
        self.cur_ast_pos.upgrade().unwrap()
    }
    pub fn is_in_userdefined_stage(&self) -> bool {
        match self.get_cur_ast_binding().borrow().value {
            NodeType::UserDefined { .. } | NodeType::UserDefinedRegex(..) => true,
            _ => false,
        }
    }

    fn get_current_nodeval(&self) -> NodeType {
        println!("{}", self.get_cur_ast_binding().borrow().id);
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

    #[test]
    fn test_deep_clone() {
        let root = TreeNode::new_null(None);
        let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
        let child = TreeNode::new(sign_token.clone(), &root);
        sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

        let child2 = TreeNode::new(sign_token, &root);
        let types = TreeNode::new_required(NodeType::Null, &child);
        println!("hi?");
        TreeNode::add_child_cycle_safe(&types, &root);
        let cloned_root = root.borrow().deep_clone();
        let root = root.borrow();
        let cloned_root = cloned_root.borrow();
        assert_eq!(root.value, cloned_root.value);
        // TODO: make this go all the way through the tree
        for (i, child) in root.children.iter().enumerate() {
            assert_eq!(child.borrow().value, cloned_root.children[i].borrow().value);
        }
    }

    #[test]
    fn simple_full() {
        let bnf = r"
        t1 ::= t2 | t3;
        t2 ::= 'r' t3;
        t3 ::= 'a';
        ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        let mut cursor = TreeCursor::new(&root);
        assert_eq!("r", cursor.advance('r').unwrap());
        assert_eq!("a", cursor.advance('a').unwrap());
        assert!(cursor.is_done());
    }
}
