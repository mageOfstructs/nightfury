use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::get_id;
use crate::NameShortener;

type FSMNodeWrapper = Rc<RefCell<FSMNode>>;
trait FSMOp = FnMut(&mut HashSet<NodeId>, &FSMNodeWrapper, &FSMNodeWrapper, &mut isize) -> bool;
trait FSMUnsafe = Fn(&mut HashSet<NodeId>, &FSMNode, &FSMNode, &mut isize) -> bool;
pub trait CycleAwareOp<T> {
    fn walk_fsm(&self, op: &mut T, greedy: bool, depth_search: bool) -> Option<FSMNodeWrapper>;
    fn walk_fsm_internal(
        &self,
        op: &mut T,
        greedy: bool,
        depth_search: bool,
        visited_nodes: &mut HashSet<NodeId>,
    ) -> Option<FSMNodeWrapper>;
    fn walk_fsm_breadth(&self, op: &mut T, greedy: bool) -> Option<FSMNodeWrapper> {
        self.walk_fsm(op, greedy, false)
    }
    fn walk_fsm_depth(&self, op: &mut T, greedy: bool) -> Option<FSMNodeWrapper> {
        self.walk_fsm(op, greedy, true)
    }
    // no this is not a workaround I swear
    // Good things *can* come out of having something like this as constructs like Repeats need
    // to loop back
    fn walk_fsm_allow_cycle_to_self(
        &self,
        op: &mut T,
        greedy: bool,
        depth_search: bool,
    ) -> Option<FSMNodeWrapper> {
        self.walk_fsm_internal(op, greedy, depth_search, &mut HashSet::new())
    }
}

// TODO: refactor
impl<T> CycleAwareOp<T> for FSMNode
where
    T: FSMUnsafe,
{
    fn walk_fsm_internal(
        &self,
        op: &mut T,
        greedy: bool,
        depth_search: bool,
        visited_nodes: &mut HashSet<NodeId>,
    ) -> Option<FSMNodeWrapper> {
        let children = self.children.clone();
        let mut c_idx = 0;
        for child in children.iter() {
            if !visited_nodes.contains(&child.borrow().id) {
                visited_nodes.insert(child.borrow().id);
                if depth_search {
                    if (greedy || child.borrow().is_null())
                        && let Some(child) = child.borrow().walk_fsm_internal(
                            op,
                            greedy,
                            depth_search,
                            visited_nodes,
                        )
                    {
                        return Some(child);
                    }
                }
                if op(visited_nodes, self, &child.borrow(), &mut c_idx) {
                    return Some(child.clone());
                }
            }
            c_idx += 1;
        }
        if !depth_search {
            for child in children.iter() {
                if (greedy || child.borrow().is_null())
                    && let Some(child) =
                        child
                            .borrow()
                            .walk_fsm_internal(op, greedy, depth_search, visited_nodes)
                {
                    return Some(child);
                }
            }
        }
        None
    }
    fn walk_fsm(&self, op: &mut T, greedy: bool, depth_search: bool) -> Option<FSMNodeWrapper> {
        let mut visisted_nodes = HashSet::new();
        visisted_nodes.insert(self.id);
        self.walk_fsm_internal(op, greedy, depth_search, &mut visisted_nodes)
    }
}

impl<T> CycleAwareOp<T> for FSMNodeWrapper
where
    T: FSMOp,
{
    fn walk_fsm_internal(
        &self,
        op: &mut T,
        greedy: bool,
        depth_search: bool,
        visited_nodes: &mut HashSet<NodeId>,
    ) -> Option<FSMNodeWrapper> {
        let children = self.borrow().children.clone();
        let mut c_idx = 0;
        for child in children.iter() {
            if !visited_nodes.contains(&child.borrow().id) {
                if depth_search {
                    visited_nodes.insert(child.borrow().id);
                    if (greedy || child.borrow().is_null())
                        && let Some(child) =
                            child.walk_fsm_internal(op, greedy, depth_search, visited_nodes)
                    {
                        return Some(child);
                    }
                }
                if op(visited_nodes, self, child, &mut c_idx) {
                    return Some(child.clone());
                }
            }
            c_idx += 1;
        }
        if !depth_search {
            for child in children.iter() {
                if !visited_nodes.contains(&child.borrow().id) {
                    visited_nodes.insert(child.borrow().id);
                    if (greedy || child.borrow().is_null())
                        && let Some(child) =
                            child.walk_fsm_internal(op, greedy, depth_search, visited_nodes)
                    {
                        return Some(child);
                    }
                }
            }
        }
        None
    }
    fn walk_fsm(&self, op: &mut T, greedy: bool, depth_search: bool) -> Option<FSMNodeWrapper> {
        let mut visisted_nodes = HashSet::new();
        visisted_nodes.insert(self.borrow().id);
        self.walk_fsm_internal(op, greedy, depth_search, &mut visisted_nodes)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyword {
    pub short: String,
    pub expanded: String,
    pub closing_token: Option<String>,
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

use NodeType::*;
use debug_print::debug_println;
use regex::Regex;

type NodeId = usize;
#[derive(Debug, Clone, PartialEq)]
pub struct FSMNode {
    id: NodeId,
    is_done: bool,
    pub value: NodeType,
    pub children: Vec<Rc<RefCell<FSMNode>>>,
}

impl Default for FSMNode {
    fn default() -> Self {
        Self {
            id: get_id(),
            is_done: false,
            value: Null,
            children: Vec::new(),
        }
    }
}

impl FSMNode {
    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn is_done(&self) -> bool {
        self.is_done
    }
    pub fn set_is_done(&mut self, val: bool) {
        self.is_done = val;
    }
    pub fn is_null(&self) -> bool {
        if let Null = self.value { true } else { false }
    }
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
    pub fn deep_clone(&self) -> Rc<RefCell<Self>> {
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
    fn has_direct_child(&self, id: usize) -> bool {
        self.children.iter().find(|c| c.borrow().id == id).is_some()
    }
    pub fn node_cnt(this: &Rc<RefCell<FSMNode>>) -> usize {
        let mut ret = 1; // one root node
        this.walk_fsm_depth(
            &mut |_, parent, _, _| {
                ret += parent.borrow().children.len();
                false
            },
            true,
        );
        ret
    }
    fn get_direct_child_dups(&self) -> Vec<usize> {
        let mut ids = HashSet::new();
        let mut ret = Vec::new();
        self.children.iter().enumerate().for_each(|(i, c)| {
            if ids.contains(&c.borrow().id) {
                ret.push(i);
            } else {
                ids.insert(c.borrow().id);
            }
        });
        ret
    }
    pub fn minify(this: &Rc<RefCell<FSMNode>>) {
        debug_println!("before minify:");
        this.borrow().dbg();
        let mut cycle_translation_table = HashMap::new();
        this.walk_fsm_depth(
            &mut |_, parent, child, childidx| {
                // TODO: figure out why the parent check needs to be here
                if parent.borrow().is_null()
                    && child.borrow().is_null()
                    && parent.borrow().children.len() == 1
                {
                    cycle_translation_table.insert(child.borrow().id, parent.clone());
                    parent.borrow_mut().children.remove(*childidx as usize);
                    for child in &child.borrow().children {
                        debug_println!("CID: {}", child.borrow().short_id());
                        if child.borrow().id == parent.borrow().id
                            || parent.borrow().has_direct_child(child.borrow().id)
                        {
                            continue;
                        }
                        parent.borrow_mut().children.push(child.clone());
                    }
                    child.borrow_mut().children.clear();
                    *childidx -= 1;
                }
                let pborrow = parent.borrow();
                let dup_idxs = pborrow.get_direct_child_dups();
                drop(pborrow);
                dup_idxs.iter().for_each(|ci| {
                    parent.borrow_mut().children.remove(*ci);
                });
                false
            },
            true,
        );
        // fix any broken pointers the last op may have created
        this.walk_fsm_depth(
            &mut |_, parent, child, childidx| {
                if let Some(new_child) = cycle_translation_table.get(&child.borrow().id) {
                    if new_child.borrow().id == parent.borrow().id
                        || parent.borrow().has_direct_child(new_child.borrow().id)
                    {
                        parent.borrow_mut().children.remove(*childidx as usize);
                        return false;
                    }
                    parent.borrow_mut().children[*childidx as usize] = new_child.clone();
                }
                false
            },
            true,
        );
        debug_println!("after minify:");
        this.borrow().dbg();
    }

    #[deprecated]
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

    #[deprecated]
    pub fn do_stuff_cycle_aware_non_greedy(
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
    pub fn has_useful_children(&self) -> bool {
        self.walk_fsm_breadth(
            &mut |_, _, c, _| match c.value {
                Null => false,
                _ => true,
            },
            false,
        )
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
        let ret = Rc::new(RefCell::new(Self {
            value: Null,
            children: Vec::new(),
            ..Default::default()
        }));
        if let Some(parent) = parent {
            parent.borrow_mut().add_child(&ret);
        }
        ret
    }

    pub fn short_id(&self) -> String {
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
        Rc::new(RefCell::new(self.clone())).walk_fsm(
            &mut |visited_nodes, _, child, _| {
                if discovered_leaves
                    .iter()
                    .find(|dl| dl.borrow().id == child.borrow().id)
                    .is_some()
                {
                    return false;
                }
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
                            "adding node {:?} {} (has_only_cycles case)",
                            child.borrow().value,
                            child.borrow().short_id()
                        );
                        discovered_leaves.push(child.clone());
                    }
                }
                false
            },
            true,
            false,
        );
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
        self.walk_fsm_depth(
            &mut |visited_nodes, _, child, _| {
                let mut ret = true;
                // avoid going back to a node previously visited so do_stuff_cycle_aware doesn't return
                // a false negative
                for child in &child.children {
                    if !visited_nodes.contains(&child.borrow().id) {
                        ret = false;
                        break;
                    }
                }
                ret
            },
            true,
        )
    }
    pub fn dbg(&self) {
        #[cfg(debug_assertions)]
        self.dbg_internal(0, &mut HashSet::new());
    }

    pub fn new(value: NodeType, parent: &Rc<RefCell<FSMNode>>) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value,
            children: Vec::new(),
            ..Default::default()
        }));
        parent.borrow_mut().add_child(&ret);
        ret
    }

    pub fn new_required(value: NodeType, parent: &Rc<RefCell<FSMNode>>) -> Rc<RefCell<Self>> {
        let ret = Rc::new(RefCell::new(Self {
            value,
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
            children: Vec::new(),
            ..Default::default()
        }))
    }

    pub fn new_keyword_with_parent(
        expanded_name: String,
        parent: Rc<RefCell<FSMNode>>,
    ) -> Rc<RefCell<Self>> {
        let ret = Self::new_keyword(expanded_name);
        FSMNode::add_child_cycle_safe(&parent, &ret);
        ret
    }
    pub fn find_node_with_code(&self, short: &str) -> Option<Rc<RefCell<FSMNode>>> {
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

    pub fn check_for_conflicts(&self, short: &str) -> bool {
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
        self.walk_fsm_breadth(
            &mut |_, _, child, _| {
                println!("awa?");
                match &child.value {
                    Keyword(Keyword { short: nshort, .. }) if short.starts_with(nshort) => {
                        return true;
                    }
                    _ => false,
                }
            },
            false,
        )
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
