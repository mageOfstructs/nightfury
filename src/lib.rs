#![feature(let_chains)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

pub mod frontend;

static mut CNT: usize = 0;
fn get_id() -> usize {
    let ret;
    unsafe {
        ret = CNT;
        CNT += 1;
    }
    return ret;
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct NodeValue {
    pub ntype: NodeType,
    pub is_done: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FSMNode {
    id: usize,
    is_done: bool,
    value: NodeType,
    parent: Option<Rc<RefCell<FSMNode>>>,
    children: Vec<Rc<RefCell<FSMNode>>>,
}

impl Default for FSMNode {
    fn default() -> Self {
        Self {
            id: get_id(),
            is_done: false,
            value: Null,
            parent: None,
            children: Vec::new(),
        }
    }
}

impl FSMNode {
    fn deep_clone_internal(
        stub: &Rc<RefCell<Self>>,
        old: &FSMNode,
        visited_nodes: &mut HashMap<usize, Rc<RefCell<FSMNode>>>,
    ) -> Rc<RefCell<Self>> {
        for child in &old.children {
            if !visited_nodes.contains_key(&child.borrow().id) {
                let clone = Rc::new(RefCell::new(Self {
                    value: child.borrow().value.clone(),
                    ..Default::default()
                }));
                visited_nodes.insert(child.borrow().id, clone.clone());
                FSMNode::deep_clone_internal(&clone, &child.borrow(), visited_nodes);
                stub.borrow_mut().children.push(clone);
            } else {
                stub.borrow_mut()
                    .children
                    .push(visited_nodes.get(&child.borrow().id).unwrap().clone());
            }
        }
        stub.clone()
    }
    fn deep_clone(&self) -> Rc<RefCell<Self>> {
        debug_println!("Deep cloning node {}", self.short_id());
        let ret = Rc::new(RefCell::new(Self {
            value: self.value.clone(),
            ..Default::default()
        }));
        let mut visited_nodes = HashMap::new();
        visited_nodes.insert(self.id, ret.clone());
        let ret = FSMNode::deep_clone_internal(&ret, self, &mut visited_nodes);
        debug_println!("Finish deep clone:");
        ret.borrow().dbg();
        ret
    }
    fn minify(this: &Rc<RefCell<FSMNode>>) {
        // let mut cycle_translation_table = HashMap::new();
        this.borrow()
            .do_stuff_cycle_aware(&mut |visited_nodes, parent, child| {
                if let Null = parent.value
                    && let Null = child.borrow().value
                    // leave cycles alone for now
                    && !visited_nodes.contains(&child.borrow().id)
                {
                    // cycle_translation_table.insert(child.borrow().id, parent);
                }
                false
            });
    }
    fn do_stuff_cycle_aware(
        &self,
        op: &mut impl FnMut(&mut HashSet<usize>, &FSMNode, Rc<RefCell<FSMNode>>) -> bool,
    ) -> Option<Rc<RefCell<FSMNode>>> {
        let mut visited_nodes = HashSet::new();
        visited_nodes.insert(self.id);
        self.do_stuff_cycle_aware_internal(op, &mut visited_nodes)
    }
    fn do_stuff_cycle_aware_internal(
        &self,
        op: &mut impl FnMut(&mut HashSet<usize>, &FSMNode, Rc<RefCell<FSMNode>>) -> bool,
        visited_nodes: &mut HashSet<usize>,
    ) -> Option<Rc<RefCell<FSMNode>>> {
        for child in &self.children {
            if !visited_nodes.contains(&child.borrow().id) {
                visited_nodes.insert(child.borrow().id);
                if op(visited_nodes, self, child.clone()) {
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
        op: &mut impl FnMut(Rc<RefCell<FSMNode>>) -> bool,
    ) -> Option<Rc<RefCell<FSMNode>>> {
        // TODO: figure out why this breaks things when you start the hashset off with the id of
        // self
        self.do_stuff_cycle_aware_non_greedy_internal(op, &mut HashSet::new())
    }
    fn do_stuff_cycle_aware_non_greedy_internal(
        &self,
        op: &mut impl FnMut(Rc<RefCell<FSMNode>>) -> bool,
        visited_nodes: &mut HashSet<usize>,
    ) -> Option<Rc<RefCell<FSMNode>>> {
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
        self.do_stuff_cycle_aware(&mut |_, _, c| match c.borrow().value {
            Null => false,
            _ => true,
        })
        .is_some()
    }

    pub fn get_last_child(&self) -> Option<Rc<RefCell<FSMNode>>> {
        self.children.last().cloned()
    }
    pub fn add_child(&mut self, child: &Rc<RefCell<FSMNode>>) {
        while self.handle_potential_conflict(child) {}
        self.children.push(Rc::clone(&child));
    }
    pub fn add_child_cycle_safe(this: &Rc<RefCell<FSMNode>>, child: &Rc<RefCell<FSMNode>>) {
        while this.borrow().handle_potential_conflict(child) {}
        this.borrow_mut().children.push(Rc::clone(&child));
    }
    pub fn new_null(parent: Option<&Rc<RefCell<FSMNode>>>) -> Rc<RefCell<Self>> {
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
        format!("{:#x}", self.id)
    }

    fn dbg_internal(&self, indent: usize, visited_nodes: &mut HashSet<usize>) {
        println!("{}{:?} {}", " ".repeat(indent), self.value, self.short_id());
        visited_nodes.insert(self.id);
        for child in self.children.iter() {
            if !visited_nodes.contains(&child.borrow().id) {
                child.borrow().dbg_internal(indent + 4, visited_nodes);
            } else {
                println!(
                    "{}Cycle to {}",
                    " ".repeat(indent + 4),
                    child.borrow().short_id()
                );
            }
        }
    }
    fn get_all_leaves(&self, discovered_leaves: &mut Vec<Rc<RefCell<FSMNode>>>) {
        self.do_stuff_cycle_aware(&mut |visited_nodes, _, child| {
            if child.borrow().children.is_empty() {
                debug_println!(
                    "adding node {:?} {}",
                    child.borrow().value,
                    child.borrow().short_id()
                );
                discovered_leaves.push(child.clone());
            } else {
                let mut has_only_cycles = true;
                for child in &child.borrow().children {
                    if !visited_nodes.contains(&child.borrow().id) {
                        has_only_cycles = false;
                        break;
                    }
                }
                if has_only_cycles {
                    debug_println!(
                        "adding node {:?} {}",
                        child.borrow().value,
                        child.borrow().short_id()
                    );
                    discovered_leaves.push(child.clone());
                }
            }
            false
        });
    }
    pub fn add_child_to_all_leaves(this: &Rc<RefCell<FSMNode>>, child: &Rc<RefCell<FSMNode>>) {
        let mut leaves = Vec::new();
        this.borrow().get_all_leaves(&mut leaves);
        while let Some(node) = leaves.pop() {
            FSMNode::add_child_cycle_safe(&node, child);
            // NOTE: hopefully this isn't needed anymore
            // if node.borrow().children.is_empty() {
            //     FSMNode::add_child_cycle_safe(&node, child);
            // }
        }
        if this.borrow().children.is_empty() {
            FSMNode::add_child_cycle_safe(&this, child);
        }
    }

    pub fn race_to_leaf(&self) -> Option<Rc<RefCell<FSMNode>>> {
        self.do_stuff_cycle_aware(&mut |visited_nodes, _, child| {
            let mut ret = true;
            // avoid going back to a node previously visited so do_stuff_cycle_aware doesn't return
            // a false negative
            for child in &child.borrow().children {
                if !visited_nodes.contains(&child.borrow().id) {
                    ret = false;
                    break;
                }
            }
            ret
        })
    }
    pub fn dbg(&self) {
        #[cfg(debug_assertions)]
        self.dbg_internal(0, &mut HashSet::new());
    }

    pub fn new(value: NodeType, parent: &Rc<RefCell<FSMNode>>) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value,
            parent: Some(Rc::clone(parent)),
            children: Vec::new(),
            ..Default::default()
        }));
        parent.borrow_mut().add_child(&ret);
        ret
    }

    pub fn new_required(value: NodeType, parent: &Rc<RefCell<FSMNode>>) -> Rc<RefCell<Self>> {
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
        parent: Rc<RefCell<FSMNode>>,
    ) -> Rc<RefCell<Self>> {
        let ret = Self::new_keyword(expanded_name);
        ret.borrow_mut().parent = Some(Rc::clone(&parent));
        FSMNode::add_child_cycle_safe(&parent, &ret);
        ret
    }
    fn find_node_with_code(&self, short: &str) -> Option<Rc<RefCell<FSMNode>>> {
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

    fn get_conflicting_node(&self, short: &str) -> Option<Rc<RefCell<FSMNode>>> {
        self.do_stuff_cycle_aware_non_greedy(&mut |child: Rc<RefCell<FSMNode>>| {
            println!("awa?");
            match &child.borrow().value {
                Keyword(Keyword { short: nshort, .. }) if short.starts_with(nshort) => {
                    return true;
                }
                _ => false,
            }
        })
    }
    fn handle_potential_conflict_internal(&self, child: &Rc<RefCell<FSMNode>>) -> bool {
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
    pub fn handle_potential_conflict(&self, child: &Rc<RefCell<FSMNode>>) -> bool {
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
            let mut visited_nodes = HashSet::new();
            // iterate over every child and return true if at least one had a conflict
            for child in &child_borrow.children {
                if !visited_nodes.contains(&child.borrow().id) {
                    visited_nodes.insert(child.borrow().id);
                    if self.handle_potential_conflict_internal(&child) {
                        let mut mut_child = child.borrow_mut();
                        if let Keyword(k) = &mut mut_child.value {
                            k.short = NameShortener::expand(Some(&k.short), &k.expanded);
                        }
                        ret = true;
                    }
                }
            }
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

type InternalCursor = Weak<RefCell<FSMNode>>;
pub struct FSMCursor {
    cur_ast_pos: InternalCursor,
    input_buf: String,
    unfinished_nodes: Vec<InternalCursor>,
}

impl FSMCursor {
    pub fn new(fsm_root: &Rc<RefCell<FSMNode>>) -> Self {
        Self {
            cur_ast_pos: Rc::downgrade(fsm_root),
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
    fn search_for_userdefs(&self, treenode: &Rc<RefCell<FSMNode>>) -> Option<Rc<RefCell<FSMNode>>> {
        treenode
            .borrow()
            .do_stuff_cycle_aware_non_greedy(&mut |child| match child.borrow().value {
                UserDefined { .. } | UserDefinedRegex { .. } => true,
                _ => false,
            })
    }
    pub fn search_rec(&self, treenode: &Rc<RefCell<FSMNode>>) -> Option<Rc<RefCell<FSMNode>>> {
        self.search_rec_internal(treenode, false)
    }
    pub fn search_rec_internal(
        &self,
        treenode: &Rc<RefCell<FSMNode>>,
        best_effort: bool,
    ) -> Option<Rc<RefCell<FSMNode>>> {
        debug_println!(
            "search_rec at {:?} {}",
            treenode.borrow().value,
            treenode.borrow().short_id()
        );
        let mut keyword_match = None;
        let mut potential_matches = 0;
        let mut visited_keywords = 0;
        let mut last_keyword = None;
        treenode
            .borrow()
            .do_stuff_cycle_aware_non_greedy(&mut |child| {
                let node_val = &child.borrow().value;
                debug_println!(
                    "search_rec closure at {:?} {}",
                    child.borrow().value,
                    child.borrow().short_id()
                );
                match node_val {
                    NodeType::Keyword(Keyword { short, .. }) => {
                        if short.starts_with(&self.input_buf) {
                            debug_println!("{:?}", child.borrow().value);
                            debug_println!("{short} == {}", self.input_buf);
                            keyword_match = Some(child.clone());
                            potential_matches += 1;
                            potential_matches > 1
                        } else {
                            // bandaid logic
                            visited_keywords += 1;
                            if visited_keywords == 1 {
                                last_keyword = Some(child.clone());
                            }
                            false
                        }
                    }
                    _ => false,
                }
            });
        debug_println!("pm: {potential_matches}");
        if keyword_match.is_some() && potential_matches == 1 {
            return keyword_match;
        }

        debug_println!("vk: {visited_keywords}");
        if visited_keywords == 1 && best_effort {
            // probably what the user wants
            return last_keyword;
        }

        let userdef_match = self.search_for_userdefs(treenode);
        if userdef_match.is_some() {
            return userdef_match;
        }
        None
    }
    pub fn advance(&mut self, input: char) -> Option<String> {
        let binding = self.get_cur_ast_binding();
        let borrow = binding.borrow();
        debug_println!(
            "advance with cursor {:?} {}",
            borrow.value,
            borrow.short_id()
        );
        self.input_buf.push(input);
        // TODO: refactor
        if borrow.is_done {
            let res = self.search_rec(&binding);
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
                    NodeType::UserDefinedRegex(_) => None,
                    _ => unreachable!(),
                };
            }
        }
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
                    drop(borrow);
                    binding.borrow_mut().is_done = true;
                    self.input_buf.clear();
                    self.input_buf.push(input);
                    let next_node = self.search_rec_internal(&binding, true);
                    if next_node.is_none() {
                        println!("No node found");
                        self.input_buf.clear();
                        return Some(input.to_string());
                    }
                    let next_node = next_node.unwrap();
                    self.update_cursor(&next_node);
                    let ret = if let NodeType::Keyword(Keyword {
                        short,
                        expanded,
                        closing_token: None,
                        ..
                    }) = &next_node.borrow().value
                    {
                        if short.starts_with(input) {
                            Some(expanded.clone())
                        } else {
                            // FIXME: this is terrible logic
                            if let Some(next_node) = self.search_rec_internal(&next_node, true) {
                                self.update_cursor(&next_node);
                                if let Keyword(Keyword {
                                    expanded,
                                    closing_token: None,
                                    ..
                                }) = &next_node.borrow().value
                                // && short.starts_with(input)
                                {
                                    Some(expanded.clone())
                                } else {
                                    Some(input.to_string())
                                }
                            } else {
                                Some(input.to_string())
                            }
                        }
                    } else {
                        Some(input.to_string())
                    };
                    self.input_buf.clear();
                    return ret;
                }
            }
            _ => {
                let res = self.search_rec(&binding);
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
                        NodeType::UserDefinedRegex(_) => None,
                        _ => unreachable!(),
                    };
                }
            }
        }
        None
    }

    fn update_cursor(&mut self, node: &Rc<RefCell<FSMNode>>) {
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
            UserDefinedRegex(..) | UserDefined { .. } if !binding.is_done => false,
            _ => binding.children.is_empty() || !binding.has_useful_children(),
        }
    }

    fn get_cur_ast_binding(&self) -> Rc<RefCell<FSMNode>> {
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
    fn find_node_with_code(&self, short: &str) -> Option<Rc<RefCell<FSMNode>>> {
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
        let root = FSMNode::new_keyword("int".to_string());
        let _other = FSMNode::new_keyword_with_parent("asdf".to_string(), root.clone());
        assert_eq!(root.borrow().children.len(), 1);
    }

    #[test]
    fn simple_cursor_steps() {
        let root = FSMNode::new_null(None);
        let second = FSMNode::new_keyword_with_parent("int".to_string(), root.clone());
        FSMNode::new_keyword_with_parent("asdf".to_string(), second.clone());
        let mut cursor = FSMCursor::new(&root);
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
        let root = FSMNode::new_null(None);
        let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
        let child = FSMNode::new(sign_token.clone(), &root);
        sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

        let child2 = FSMNode::new(sign_token, &root);
        let types = FSMNode::new_required(NodeType::Null, &child);

        let int = FSMNode::new_keyword_with_parent("int".to_string(), types.clone());
        let float = FSMNode::new_keyword_with_parent("short".to_string(), types.clone());
        child.borrow_mut().add_child(&types);
        child2.borrow_mut().add_child(&types);

        assert!(root.borrow().check_for_conflicts("s"));
        assert!(child2.borrow().check_for_conflicts("s"));
        assert!(types.borrow().check_for_conflicts("s"));
        assert!(!int.borrow().check_for_conflicts("s"));
    }

    #[test]
    fn test_keyword_matching() {
        let root = FSMNode::new_null(None);
        let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
        let child = FSMNode::new(sign_token.clone(), &root);
        sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

        let child2 = FSMNode::new(sign_token, &root);
        let types = FSMNode::new_required(NodeType::Null, &child);

        let int = FSMNode::new_keyword_with_parent("int".to_string(), types.clone());
        let float = FSMNode::new_keyword_with_parent("short".to_string(), types.clone());
        root.borrow_mut().add_child(&types);
        child2.borrow_mut().add_child(&types);

        let mut cursor = FSMCursor::new(&root);
        assert!(cursor.advance('s').is_none());
        assert!(cursor.advance('h').is_some());
        let mut cursor = FSMCursor::new(&root);
        assert!(cursor.advance('u').is_some());
        assert!(cursor.advance('s').is_some());
    }

    #[test]
    fn test_deep_clone() {
        let root = FSMNode::new_null(None);
        let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
        let child = FSMNode::new(sign_token.clone(), &root);
        sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

        let child2 = FSMNode::new(sign_token, &root);
        let types = FSMNode::new_required(NodeType::Null, &child);
        println!("hi?");
        FSMNode::add_child_cycle_safe(&types, &root);
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
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("r", cursor.advance('r').unwrap());
        assert_eq!("a", cursor.advance('a').unwrap());
        assert!(cursor.is_done());
    }

    #[test]
    fn test_optional() {
        let bnf = r"
        t1 ::= 't' ( 'e' t2 )? 't';
        t2 ::= 's' ( t3 )?;
        t3 ::= 'a';
        ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("t", cursor.advance('t').unwrap());
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());

        let mut cursor = FSMCursor::new(&root);
        assert_eq!("t", cursor.advance('t').unwrap());
        assert_eq!("e", cursor.advance('e').unwrap());
        assert_eq!("s", cursor.advance('s').unwrap());
        assert_eq!("a", cursor.advance('a').unwrap());
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());

        let mut cursor = FSMCursor::new(&root);
        assert_eq!("t", cursor.advance('t').unwrap());
        assert_eq!("e", cursor.advance('e').unwrap());
        assert_eq!("s", cursor.advance('s').unwrap());
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());
    }

    #[test]
    fn test_optional2() {
        let bnf = r"
        t1 ::= 'te' t2 't';
        t2 ::= ( 's' )?; 
        ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("te", cursor.advance('t').unwrap());
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("te", cursor.advance('t').unwrap());
        assert_eq!("s", cursor.advance('s').unwrap());
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());
    }

    // you are the bane of my existence. If I ever had the chance to erase
    // something from the universe permanently, I would erase recursive bnf terminal definitions.
    // There is no reason this is so incredibly hard to parse.
    #[test]
    fn test_sql() {
        let bnf = r"
        query ::= select | insert;
        select ::= 'SELECT' '*' | collist 'FROM' #'^.*;$';
        insert ::= 'INSERT INTO' #'^.* $' 'VALUES' '(' collist ')';
        collist ::= col ( ',' collist )?;
        col ::= #'^.*[, ]$';
    ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("SELECT", cursor.advance('S').unwrap());
        assert_eq!(None, cursor.advance('a'));
        assert_eq!(",", cursor.advance(',').unwrap());
        assert_eq!(None, cursor.advance('b'));
        cursor.advance(' ');
        assert_eq!("FROM", cursor.advance('F').unwrap());
        cursor.advance('a');
        assert_eq!(";", cursor.advance(';').unwrap());
        assert!(cursor.is_done());
    }

    #[test]
    fn test_repeat() {
        let bnf = r"
        t1 ::= 't' { 'e' } 'st';
    ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        // >:3
        for i in 0..=30 {
            let mut cursor = FSMCursor::new(&root);
            assert_eq!("t", cursor.advance('t').unwrap());
            for _ in 0..i {
                assert_eq!("e", cursor.advance('e').unwrap());
            }
            assert_eq!("st", cursor.advance('s').unwrap());
            assert!(cursor.is_done());
        }
    }

    #[test]
    fn test_terminal() {
        let terms: usize = 100;
        let mut bnf = String::with_capacity(terms * 14);
        for i in 1..terms {
            bnf.push_str(&format!("t{i:0>3} ::= t{:0>3};\n", i + 1));
        }
        bnf.push_str(&format!("t{terms:0>3} ::= 't' 'st';"));
        println!("{bnf}");
        let root = frontend::create_graph_from_ebnf(&bnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("t", cursor.advance('t').unwrap());
        assert_eq!("st", cursor.advance('s').unwrap());
        assert!(cursor.is_done());
    }

    fn util_check_str(root: &Rc<RefCell<FSMNode>>, str: &str) {
        let mut cursor = FSMCursor::new(root);
        for char in str.chars() {
            cursor.advance(char);
        }
        assert!(cursor.is_done())
    }

    #[test]
    fn test_combi() {
        let bnf = r"
        t1 ::= ( 'u' | 'a' | 'o' ) { 'w' | 'v' } ( 'u' | 'a' | 'o' );
    ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        util_check_str(&root, "uwu");
        util_check_str(&root, "owo");
        util_check_str(&root, "owwwwwo");
        util_check_str(&root, "owwwwwu");
    }

    #[test]
    fn test_repeat_multiple() {
        let bnf = r"
        t1 ::= 't' t2 't';
        t2 ::= 'oas' | ( 'e' { 'a' 's' } );
    ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("t", cursor.advance('t').unwrap());
        assert_eq!("oas", cursor.advance('o').unwrap());
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());

        let mut cursor = FSMCursor::new(&root);
        assert_eq!("t", cursor.advance('t').unwrap());
        assert_eq!("e", cursor.advance('e').unwrap());
        assert_eq!("a", cursor.advance('a').unwrap());
        assert_eq!("s", cursor.advance('s').unwrap());
        assert_eq!("a", cursor.advance('a').unwrap());
        assert_eq!("s", cursor.advance('s').unwrap());
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());
    }

    #[test]
    fn test_minify() {
        let root = FSMNode::new_null(None);
        let child = FSMNode::new_null(Some(&root));
        let child = FSMNode::new_keyword_with_parent("asdf".to_string(), child);
        // minify
        assert_eq!(Null, root.borrow().value);
        assert_eq!(
            Keyword(Keyword::new("asdf".to_string(), None)),
            root.borrow().children[0].borrow().value
        );
        assert_eq!(1, root.borrow().children.len())
    }
}
