use super::FSMRc;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::read_to_string;
use std::str::pattern::Pattern;

use super::FSMLock;
use super::get_id;
use crate::NameShortener;
use crate::esc_seq::resolve_escape_sequences;

pub type FSMNodeWrapper = FSMRc<FSMLock<FSMNode>>;
trait FSMOp = FnMut(&mut HashSet<NodeId>, &FSMNodeWrapper, &FSMNodeWrapper, &mut isize) -> bool;
trait FSMUnsafe = FnMut(&mut HashSet<NodeId>, &FSMNode, &FSMNode, &mut isize) -> bool;
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
        let children = &self.children;
        let mut c_idx = 0;
        for child in children {
            if !visited_nodes.contains(&child.borrow().id) {
                if depth_search {
                    visited_nodes.insert(child.borrow().id);
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
                    && !visited_nodes.contains(&child.borrow().id)
                {
                    visited_nodes.insert(child.borrow().id);

                    if let Some(child) =
                        child
                            .borrow()
                            .walk_fsm_internal(op, greedy, depth_search, visited_nodes)
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
        // TODO: should use a for loop using c_idx instead
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

// FIXME: the strcpys take up a decent amount of time, maybe expanded can be made a reference?
#[derive(Debug, Clone, PartialEq, Default)]
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

#[derive(Debug, Clone)]
pub enum NodeType {
    Keyword(Keyword),
    UserDefinedCombo(Regex, Vec<char>),
    Null,
}

impl PartialEq for NodeType {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Keyword(k) => match other {
                Keyword(k2) => k.eq(k2),
                _ => false,
            },
            UserDefinedCombo(r, f) => match other {
                UserDefinedCombo(r2, f2) => r.as_str().eq(r2.as_str()) && f.eq(f2),
                _ => false,
            },
            Null => matches!(other, Null),
        }
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
    pub children: Vec<FSMRc<FSMLock<FSMNode>>>,
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
    #[inline]
    pub fn is_keyword(&self) -> bool {
        matches!(self.value, Keyword(_))
    }
    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self.value, Null)
    }
    #[inline]
    pub fn is_userdef(&self) -> bool {
        matches!(self.value, UserDefinedCombo(_, _))
    }
    fn deep_clone_internal(
        stub: &FSMNodeWrapper,
        old: &FSMNode,
        visited_nodes: &mut HashMap<usize, FSMNodeWrapper>,
    ) -> FSMRc<FSMLock<Self>> {
        for child in &old.children {
            if let std::collections::hash_map::Entry::Vacant(e) =
                visited_nodes.entry(child.borrow().id)
            {
                let clone = FSMRc::new(FSMLock::new(Self {
                    value: child.borrow().value.clone(),
                    ..Default::default()
                }));
                e.insert(clone.clone());
                FSMNode::deep_clone_internal(&clone, &child.borrow(), visited_nodes);
                stub.borrow_mut().children.push(clone);
            } else {
                stub.borrow_mut()
                    .children
                    .push(visited_nodes[&child.borrow().id].clone());
            }
        }
        stub.clone()
    }
    pub fn deep_clone(&self) -> FSMRc<FSMLock<Self>> {
        debug_println!("Deep cloning node {}", self.short_id());
        let ret = FSMRc::new(FSMLock::new(Self {
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
        self.children.iter().any(|c| c.borrow().id == id)
    }
    pub fn node_cnt(&self) -> usize {
        let mut ret = 1; // one root node
        self.walk_fsm_depth(
            &mut |_, parent, _, _| {
                ret += parent.children.len();
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
    pub fn set_userdef_links(this: &FSMRc<FSMLock<FSMNode>>) {
        let mut userdefs = Vec::new();
        // find all userdefs
        this.walk_fsm_breadth(
            &mut |_, p, _, _| {
                if let UserDefinedCombo(_, _) = p.borrow().value {
                    userdefs.push(FSMRc::clone(p));
                }
                false
            },
            true,
        );
        debug_println!("{}", userdefs.len());
        for userdef in userdefs {
            debug_println!(
                "{:?} {}",
                userdef.borrow().value,
                userdef.borrow().short_id()
            );
            userdef.walk_fsm_depth(
                &mut |_, _, c, _| {
                    debug_println!("{:?} {}", c.borrow().value, c.borrow().short_id());
                    if let Keyword(Keyword { short, .. }) = &c.borrow().value
                        && let UserDefinedCombo(_, fcs) = &mut userdef.borrow_mut().value
                    {
                        fcs.push(short.chars().nth(0).unwrap()); // bad handling, only possible when
                        // there aren't any conflicts
                    }
                    false
                },
                false,
            );
        }
    }
    pub fn minify(this: &FSMRc<FSMLock<FSMNode>>) {
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

    pub fn has_useful_children(&self) -> bool {
        self.walk_fsm_breadth(&mut |_, _, c, _| !matches!(c.value, Null), false)
            .is_some()
    }

    pub fn get_last_child(&self) -> Option<FSMRc<FSMLock<FSMNode>>> {
        self.children.last().cloned()
    }
    /// adds child to the children vector of self without doing collision checks first
    /// # Safety
    /// is able to create invalid fsms if collision detection is not handled elsewhere
    pub unsafe fn add_child_unsafe(&mut self, child: &FSMRc<FSMLock<FSMNode>>) {
        self.children.push(FSMRc::clone(child));
    }
    pub fn add_child(&mut self, child: &FSMRc<FSMLock<FSMNode>>) {
        while self.handle_potential_conflict(child) {}
        unsafe {
            self.add_child_unsafe(child);
        }
    }
    pub fn add_child_cycle_safe(this: &FSMRc<FSMLock<FSMNode>>, child: &FSMRc<FSMLock<FSMNode>>) {
        while this.borrow().handle_potential_conflict(child) {}
        this.borrow_mut().children.push(FSMRc::clone(child));
    }
    pub fn new_null(parent: Option<&FSMRc<FSMLock<FSMNode>>>) -> FSMRc<FSMLock<Self>> {
        let ret = FSMRc::new(FSMLock::new(Self {
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
    fn get_all_leaves(
        this: &FSMNodeWrapper,
        discovered_leaves: &mut HashMap<NodeId, FSMNodeWrapper>,
    ) {
        this.walk_fsm(
            &mut |visited_nodes, _, child, _| {
                if discovered_leaves.contains_key(&child.borrow().id) {
                    return false;
                }
                if child.borrow().children.is_empty() {
                    debug_println!(
                        "adding node {:?} {}",
                        child.borrow().value,
                        child.borrow().short_id()
                    );
                    discovered_leaves.insert(child.borrow().id, child.clone());
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
                        discovered_leaves.insert(child.borrow().id, child.clone());
                    }
                }
                false
            },
            true,
            false,
        );
    }
    pub fn add_child_to_all_leaves(this: &FSMNodeWrapper, child: &FSMNodeWrapper) {
        let mut leaves = HashMap::new();
        FSMNode::get_all_leaves(this, &mut leaves);
        let iter = leaves.values();
        for node in iter {
            FSMNode::add_child_cycle_safe(node, child);
            // NOTE: hopefully this isn't needed anymore
            // if node.borrow().children.is_empty() {
            //     FSMNode::add_child_cycle_safe(&node, child);
            // }
        }
        if this.borrow().children.is_empty() {
            FSMNode::add_child_cycle_safe(this, child);
        }
    }

    pub fn race_to_leaf(&self) -> Option<FSMRc<FSMLock<FSMNode>>> {
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

    pub fn new_id(value: NodeType, id: NodeId) -> FSMRc<FSMLock<Self>> {
        FSMRc::new(FSMLock::new(Self {
            value,
            children: Vec::new(),
            id,
            ..Default::default()
        }))
    }

    pub fn new(value: NodeType, parent: &FSMRc<FSMLock<FSMNode>>) -> FSMRc<FSMLock<Self>> {
        let ret = FSMRc::new(FSMLock::new(Self {
            value,
            children: Vec::new(),
            ..Default::default()
        }));
        parent.borrow_mut().add_child(&ret);
        ret
    }

    pub fn new_required(value: NodeType, parent: &FSMRc<FSMLock<FSMNode>>) -> FSMRc<FSMLock<Self>> {
        let ret = FSMRc::new(FSMLock::new(Self {
            value,
            children: Vec::new(),
            ..Default::default()
        }));
        parent.borrow_mut().add_child(&ret);
        ret
    }

    pub fn new_userdef(r: Regex, parent: &FSMRc<FSMLock<FSMNode>>) -> FSMRc<FSMLock<Self>> {
        let ret = FSMRc::new(FSMLock::new(Self {
            value: UserDefinedCombo(r, Vec::new()),
            children: Vec::new(),
            ..Default::default()
        }));
        FSMNode::add_child_cycle_safe(parent, &ret);
        ret
    }

    pub fn new_keyword(expanded_name: String) -> FSMRc<FSMLock<Self>> {
        FSMRc::new(FSMLock::new(Self {
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
        parent: FSMRc<FSMLock<FSMNode>>,
    ) -> FSMRc<FSMLock<Self>> {
        let ret = Self::new_keyword(expanded_name);
        FSMNode::add_child_cycle_safe(&parent, &ret);
        ret
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

    fn get_conflicting_node(&self, short: &str) -> Option<FSMRc<FSMLock<FSMNode>>> {
        self.walk_fsm_breadth(
            &mut |_, _, child, _| {
                match &child.value {
                    // shouldn't you also check nshort.starts_with(short)?
                    Keyword(Keyword { short: nshort, .. }) if short.starts_with(nshort) => true,
                    _ => false,
                }
            },
            false,
        )
    }
    fn handle_potential_conflict_internal(&self, child: &FSMRc<FSMLock<FSMNode>>) -> bool {
        let child_borrow = child.borrow();
        let mut ret = false;
        if let Keyword(Keyword { short: cshort, .. }) = &child_borrow.value
            && let Some(node) = self.get_conflicting_node(cshort)
            && node.borrow().id != child_borrow.id
        {
            node.replace_with(|node| {
                    debug_println!("Old Node: {:?} {}", node.value, node.short_id());
                    if let Keyword(keyword_struct) = &mut node.value {
                        let new_short = NameShortener::expand(
                            Some(&keyword_struct.short),
                            &keyword_struct.expanded,
                        );
                        keyword_struct.short = new_short;
                        ret = true;
                        debug_println!("New Node: {:?} {}", node.value, node.short_id());
                        node.to_owned()
                    } else {
                        panic!(
                            "What?! We got a non-keyword node from the get_conflicting_node fn! Anyways, I'm gonna snuggle some foxxos now..."
                        )
                    }
                });
        }
        ret
    }
    pub fn handle_potential_conflict(&self, child: &FSMNodeWrapper) -> bool {
        let child_borrow = child.borrow();
        if let Keyword(_) = &child_borrow.value {
            debug_println!("{:?}", self.value);
            debug_println!("{:?}", child.borrow().value);
            if self.handle_potential_conflict_internal(child) {
                drop(child_borrow);
                let mut ret = false;
                child.replace_with(|node| {
                    if let Keyword(k) = &mut node.value {
                        ret = NameShortener::expand_existing(&mut k.short, &k.expanded);
                    } else {
                        unreachable!()
                    }
                    node.to_owned()
                });
                return ret;
            }
        } else if let Null = &child_borrow.value {
            return child
                .walk_fsm_breadth(
                    &mut |_, _, child, _| {
                        if self.handle_potential_conflict_internal(child) {
                            let mut mut_child = child.borrow_mut();
                            if let Keyword(k) = &mut mut_child.value {
                                return NameShortener::expand_existing(&mut k.short, &k.expanded);
                            }
                            false
                        } else {
                            false
                        }
                    },
                    false,
                )
                .is_some();
        }
        false
    }
    pub fn dump_children(&self) {
        self.children
            .iter()
            .for_each(|child| println!("{:?}", child.borrow().value));
    }
}

pub trait ToCSV {
    const FIELD_DELIM: char = '\t';
    const ENTRY_DELIM: char = '\n';
    fn to_csv(&self) -> String;
    fn from_csv(csv: &str) -> Self;

    fn from_csv_file(path: &str) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        File::open(path)
            .and_then(read_to_string)
            .map(|fsm| Self::from_csv(&fsm))
    }
}

impl ToCSV for NodeType {
    fn to_csv(&self) -> String {
        let mut ret = match self {
            Null => "".to_owned(),
            Keyword(Keyword {
                short,
                expanded,
                closing_token,
            }) => format!(
                "{short}{}{expanded}{}",
                Self::FIELD_DELIM,
                if let Some(ct) = closing_token {
                    Self::FIELD_DELIM.to_string() + ct
                } else {
                    "".to_owned()
                }
            ),
            UserDefinedCombo(r, cts) => {
                format!(
                    "/{}{}",
                    r.as_str(),
                    cts.iter().fold(String::new(), |acc, el| {
                        format!("{acc}{}{el}", Self::FIELD_DELIM)
                    })
                )
            }
        };
        ret.push(Self::ENTRY_DELIM);
        ret
    }
    fn from_csv(csv: &str) -> Self {
        println!("csv: {csv}");
        if csv.len() < 2 {
            Null
        } else if csv.chars().nth(0).unwrap() == '/' {
            let mut iter = csv.split(Self::FIELD_DELIM);
            let regex = Regex::new(&iter.next().expect("invalid NodeType format")[1..])
                .expect("invalid Regex format");
            let mut final_tokens = Vec::with_capacity((csv.len() - regex.as_str().len()) / 2);
            final_tokens.extend(iter.map(|s| s.chars().nth(0).expect("empty closing_token field")));
            UserDefinedCombo(regex, final_tokens)
        } else {
            let mut parts = csv.split(Self::FIELD_DELIM).map(resolve_escape_sequences);
            let short = parts
                .next()
                .expect("keyword from_csv: missing short field!");
            let expanded = parts
                .next()
                .expect("keyword from_csv: missing expanded field!");
            Keyword(Keyword {
                short,
                expanded,
                closing_token: parts.next(),
            })
        }
    }
}

impl ToCSV for FSMNodeWrapper {
    fn to_csv(&self) -> String {
        let mut nodes = HashMap::new();
        self.walk_fsm_breadth(
            &mut |_, _, c, _| {
                nodes.insert(c.borrow().id, c.clone());
                false
            },
            true,
        );
        let mut ret = format!(
            "{}{}{}",
            self.borrow().id,
            Self::FIELD_DELIM,
            self.borrow().value.to_csv()
        );
        nodes.keys().for_each(|id| {
            ret.push_str(&id.to_string());
            ret.push(Self::FIELD_DELIM);
            ret.push_str(&nodes.get(id).unwrap().borrow().value.to_csv());
        });
        ret.push(Self::ENTRY_DELIM);

        ret.push_str(&self.borrow().id.to_string());
        self.borrow().children.iter().for_each(|el| {
            ret.push(Self::FIELD_DELIM);
            ret.push_str(&el.borrow().id.to_string());
        });
        ret.push(Self::ENTRY_DELIM);

        nodes.keys().for_each(|id| {
            ret.push_str(&id.to_string());
            let node = nodes.get(id).unwrap();
            node.borrow().children.iter().for_each(|el| {
                ret.push(Self::FIELD_DELIM);
                ret.push_str(&el.borrow().id.to_string());
            });
            ret.push(Self::ENTRY_DELIM);
        });
        ret
    }
    fn from_csv(csv: &str) -> Self {
        let mut iter = csv.split_indices(Self::ENTRY_DELIM);
        let mut nodes = HashMap::new();

        // TODO: refactor
        let line = iter.next().unwrap();
        println!("from_csv at line '{line:?}'");
        let mut line_iter = line.0.split_indices(Self::FIELD_DELIM);
        let id: usize = line_iter.next().unwrap().0.parse().unwrap();
        let ntype = match line_iter.next() {
            Some(nval) => NodeType::from_csv(&line.0[nval.1..]),
            None => Null,
        };

        let root = FSMNode::new_id(ntype, id);
        nodes.insert(id, root.clone());
        while let Some(part) = iter.next()
            && !part.0.is_empty()
        {
            println!("from_csv at line '{part:?}'");
            let mut line_iter = part.0.split_indices(Self::FIELD_DELIM);
            let id: usize = line_iter.next().unwrap().0.parse().unwrap();
            let ntype = match line_iter.next() {
                Some(nval) => NodeType::from_csv(&part.0[nval.1..]),
                None => Null,
            };
            nodes.insert(id, FSMNode::new_id(ntype, id));
        }
        // iter.next(); // consume separator line

        // children logic
        while let Some(part) = iter.next()
            && !part.0.is_empty()
        {
            let mut iter = part.0.split(Self::FIELD_DELIM);
            let id: usize = iter.next().unwrap().parse().unwrap();
            let parent = nodes.get(&id).unwrap();
            for part in iter {
                let c_id: NodeId = part.parse().unwrap();
                #[cfg(not(debug_assertions))]
                unsafe {
                    parent
                        .borrow_mut()
                        .add_child_unsafe(nodes.get(&c_id).unwrap());
                }
                #[cfg(debug_assertions)]
                FSMNode::add_child_cycle_safe(parent, nodes.get(&c_id).unwrap());
            }
        }
        root
    }
}

trait SplitIndicesExt {
    fn split_indices<P>(&self, pat: P) -> impl Iterator<Item = (&str, usize)>
    where
        P: Pattern;
}

impl SplitIndicesExt for &str {
    fn split_indices<P>(&self, pat: P) -> impl Iterator<Item = (&str, usize)>
    where
        P: Pattern,
    {
        let mut start = 0;
        self.match_indices(pat).map(move |(i, _)| {
            let ret = (&self[start..i], start);
            start = i + 1;
            ret
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::dbg_id;

    use super::*;

    #[test]
    fn test_csv_simple() {
        dbg_id();
        let root = FSMNode::new_keyword("int".to_string());
        let _other = FSMNode::new_keyword_with_parent("asdf".to_string(), root.clone());

        let csv = root.to_csv();
        assert_eq!("0\ti\tint\n1\ta\tasdf\n\n0\t1\n1\n", csv);
        let new_root = FSMNodeWrapper::from_csv(&csv);
        assert_eq!(root, new_root);
    }
}
