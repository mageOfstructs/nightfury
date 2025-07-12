#![feature(if_let_guard)]
#![feature(trait_alias)]
#![feature(impl_trait_in_bindings)]
#![feature(lock_value_accessors)]
#![feature(pattern)]
#![feature(buf_read_has_data_left)]
#![feature(test)]

use debug_print::debug_println;
use fsm::NodeType::{self, *};
use fsm::{CycleAwareOp, Keyword};
pub use fsm::{FSMNode, ToCSV};
use regex::Regex;
use std::cell::RefCell;
#[cfg(not(feature = "thread-safe"))]
use std::cell::{Ref, RefMut};
#[cfg(feature = "thread-safe")]
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub mod frontend;

mod fsm;
pub use fsm::FSMNodeWrapper;

pub mod protocol;

mod esc_seq;

thread_local! {
    static CNT: RefCell<usize> = RefCell::new(0);
}

fn get_id() -> usize {
    let mut ret = 0;
    CNT.with_borrow(|cnt| ret = *cnt);
    CNT.with_borrow_mut(|cnt| *cnt += 1);
    return ret;
}
fn dbg_id() {
    debug_println!("{:?}", CNT);
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
                println!("partial_match '{}' match with '{}'", hay, regex.as_str());
                return true;
            }
        }
        false
    }
}

pub fn get_test_fsm() -> FSMNodeWrapper {
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
    root
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
                // well screw you past me! It's actually vital for collisions between s1 and s2
                // where s2.starts_with(s1) applies
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
    fn expand_existing(old: &mut String, full: &str) -> bool {
        if old.len() < full.len() {
            old.push_str(&full[old.len()..old.len() + 1]);
            true
        } else {
            // needed to stop
            // handle_potential_conflict getting into an infinite loop when
            // we encounter a node where short == expanded
            // handle_potential_conflict will just always return true, which
            // will never end the while loop it's called in
            // FIXME maybe, however a fix will probably have to rewrite a lot
            // of code in frontend, removing all those pesky
            // add_child_to_all_leaves calls
            false
        }
    }
}

#[cfg(not(feature = "thread-safe"))]
type FSMRc<T> = std::rc::Rc<T>;
#[cfg(not(feature = "thread-safe"))]
type FSMWeak<T> = std::rc::Weak<T>;
#[cfg(not(feature = "thread-safe"))]
#[derive(Debug, PartialEq)]
pub struct FSMLock<T>(RefCell<T>);
#[cfg(not(feature = "thread-safe"))]
impl<T> FSMLock<T> {
    fn new(val: T) -> Self {
        Self(RefCell::new(val))
    }
    pub fn borrow(&self) -> Ref<'_, T> {
        self.0.borrow()
    }
    fn borrow_mut(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }
    fn replace_with(&self, op: impl FnOnce(&mut T) -> T) {
        self.0.replace_with(op);
    }
}

#[cfg(feature = "thread-safe")]
type FSMRc<T> = std::sync::Arc<T>;
#[cfg(feature = "thread-safe")]
type FSMWeak<T> = std::sync::Weak<T>;
#[cfg(feature = "thread-safe")]
#[derive(Debug)]
pub struct FSMLock<T>(RwLock<T>);
#[cfg(feature = "thread-safe")]
impl<T> FSMLock<T> {
    pub fn borrow(&self) -> RwLockReadGuard<'_, T> {
        self.0.read().expect("FSMLock borrow()")
    }
    fn borrow_mut(&self) -> RwLockWriteGuard<'_, T> {
        self.0.write().expect("FSMLock borrow_mut()")
    }
    fn new(val: T) -> Self {
        Self(RwLock::new(val))
    }
    fn replace_with(&self, op: impl FnOnce(&mut T) -> T) {
        let new = op(&mut self.0.write().unwrap());
        self.0.set(new).unwrap();
    }
}
#[cfg(feature = "thread-safe")]
impl<T: PartialEq> PartialEq for FSMLock<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.read().unwrap().eq(&other.0.read().unwrap())
    }
}
type InternalCursor = FSMWeak<FSMLock<FSMNode>>;
#[derive(Clone, Debug, Default)]
pub struct FSMCursor {
    root: InternalCursor,
    cur_ast_pos: InternalCursor,
    input_buf: String,
    did_revert: bool,
    unfinished_nodes: Vec<InternalCursor>,
    path: Vec<InternalCursor>,
}

/// offers more insight in what advancing the cursor did
#[derive(Debug, PartialEq)]
pub enum AdvanceResult {
    /// returned after matching a Keyword directly after a userdef
    ExpandedAfterUserdef(String),
    /// ordinary Keyword match
    Expanded(String),
    /// dead_end detection triggered and the internal state did not update
    InvalidChar,
}

impl FSMCursor {
    pub fn new(fsm_root: &FSMRc<FSMLock<FSMNode>>) -> Self {
        Self {
            root: FSMRc::downgrade(fsm_root),
            cur_ast_pos: FSMRc::downgrade(fsm_root),
            ..Default::default()
        }
    }
    /// resets the cursor back to the FSM root as if new() has just been called
    pub fn reset(&mut self) {
        self.cur_ast_pos = FSMWeak::clone(&self.root);
        self.input_buf.clear();
        self.did_revert = false;
        self.unfinished_nodes.clear();
        self.path.clear();
    }
    fn handle_userdefined_combo(&mut self, input: char, final_chars: &Vec<char>) -> Option<String> {
        let child_idx = final_chars.iter().position(|char| *char == input);
        if let Some(_) = child_idx {
            let strong_ref = self.get_cur_ast_binding();
            // let borrow = strong_ref.borrow();
            // let next_node = FSMRc::clone(&borrow.children[child_idx]);
            let mut ret = None;
            strong_ref.walk_fsm_depth(
                &mut |_, _, c, _| {
                    if let Keyword(Keyword {
                        short, expanded, ..
                    }) = &c.borrow().value
                        && short.starts_with(input)
                    {
                        println!("handle_userdefined_combo: found another keyword!");
                        self.update_cursor(&c);
                        self.input_buf.clear();
                        ret = Some(expanded.clone());
                        true
                    } else {
                        false
                    }
                },
                false,
            );
            ret
        } else {
            if let UserDefinedCombo(r, _) = self.get_current_nodeval()
                && !r.is_match(&self.input_buf)
            {
                self.did_revert = true;
                self.input_buf.pop();
            }
            None
        }
    }
    fn handle_userdefined(&mut self, input: char, final_chars: &Vec<char>) -> Option<String> {
        let child_idx = final_chars.iter().position(|char| *char == input);
        if let Some(child_idx) = child_idx {
            let strong_ref = self.get_cur_ast_binding();
            let borrow = strong_ref.borrow();
            let next_node = FSMRc::clone(&borrow.children[child_idx]);
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
    /// clears the internal buffer
    pub fn clear_inputbuf(&mut self) {
        self.input_buf.clear();
    }
    fn search_for_userdefs(
        &self,
        treenode: &FSMRc<FSMLock<FSMNode>>,
    ) -> Option<FSMRc<FSMLock<FSMNode>>> {
        treenode.borrow().walk_fsm_breadth(
            &mut |_, _, child, _| match &child.value {
                UserDefined { .. } => true,
                UserDefinedRegex(regex) | UserDefinedCombo(regex, _)
                    if regex.is_match(&self.input_buf) =>
                {
                    true
                }
                _ => false,
            },
            false,
        )
    }
    pub fn search_rec(
        &mut self,
        treenode: &FSMRc<FSMLock<FSMNode>>,
    ) -> Option<FSMRc<FSMLock<FSMNode>>> {
        self.search_rec_internal(treenode, false)
    }
    pub fn search_rec_internal(
        &mut self,
        treenode: &FSMRc<FSMLock<FSMNode>>,
        best_effort: bool, // overcomplicates things (and is probably not even used)
    ) -> Option<FSMRc<FSMLock<FSMNode>>> {
        debug_println!(
            "search_rec at {:?} {}",
            treenode.borrow().value,
            treenode.borrow().short_id()
        );
        debug_println!("search_rec input buf: {}", self.input_buf);
        let mut keyword_match = None;
        let mut potential_matches = 0;
        let mut visited_keywords = 0;
        let mut last_keyword = None;
        treenode.walk_fsm_allow_cycle_to_self(
            &mut |_, _, child, _| {
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
            },
            false,
            false,
        );
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
        if userdef_match.is_some() && potential_matches < 1 {
            return userdef_match;
        }

        // TODO: look into whether potential userdefs also need to be checked here
        // if we didn't find any potential keywords/userdef nodes
        if potential_matches == 0 {
            // don't like that this function has to take a &mut self just because of this
            self.input_buf.pop();
            self.did_revert = true;
        }
        None
    }
    /// advances the cursor's position, taking the key the user pressed last
    pub fn advancex(&mut self, input: char) -> Option<AdvanceResult> {
        let binding = self.get_cur_ast_binding();
        let borrow = binding.borrow();
        debug_println!(
            "advance with cursor {:?} {}",
            borrow.value,
            borrow.short_id()
        );
        self.input_buf.push(input);
        // TODO: refactor
        if borrow.is_done() {
            let res = self.search_rec(&binding);
            if let Some(node) = res {
                self.update_cursor(&node);
                return match &node.borrow().value {
                    NodeType::Keyword(Keyword { expanded, .. }) => {
                        self.input_buf.clear();
                        Some(AdvanceResult::Expanded(expanded.to_string()))
                    }
                    NodeType::UserDefined { final_chars } => {
                        let res = self.handle_userdefined(input, &final_chars);
                        res.map(|expanded| AdvanceResult::ExpandedAfterUserdef(expanded))
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
                    return res.map(|expanded| AdvanceResult::ExpandedAfterUserdef(expanded));
                }
            }
            NodeType::UserDefinedRegex(r) => {
                debug_println!("Checking regex against '{}'", &self.input_buf);
                if r.is_match(&self.input_buf) {
                    drop(borrow);
                    binding.borrow_mut().set_is_done(true);
                    self.input_buf.clear();
                    self.input_buf.push(input);
                    let next_node = self.search_rec_internal(&binding, true);
                    if next_node.is_none() {
                        debug_println!("No node found");
                        self.input_buf.clear();
                        return Some(AdvanceResult::ExpandedAfterUserdef(input.to_string()));
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
                    return ret.map(|exp| AdvanceResult::ExpandedAfterUserdef(exp));
                }
            }
            UserDefinedCombo(_, f) => {
                let ret = self.handle_userdefined_combo(input, f);
                if ret.is_some() {
                    return ret.map(|exp| AdvanceResult::ExpandedAfterUserdef(exp));
                }
            }
            _ => {
                let res = self.search_rec(&binding);
                if let Some(node) = res {
                    self.update_cursor(&node);
                    return match &node.borrow().value {
                        NodeType::Keyword(Keyword { expanded, .. }) => {
                            self.input_buf.clear();
                            Some(AdvanceResult::Expanded(expanded.clone()))
                        }
                        NodeType::UserDefined { final_chars } => {
                            let res = self.handle_userdefined(input, &final_chars);
                            res.map(|exp| AdvanceResult::ExpandedAfterUserdef(exp))
                        }
                        NodeType::UserDefinedRegex(_) => None,
                        NodeType::UserDefinedCombo(_, f) => {
                            let res = self.handle_userdefined_combo(input, f);
                            self.check_for_revert(
                                res.map(|exp| AdvanceResult::ExpandedAfterUserdef(exp)),
                            )
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
        self.check_for_revert(None)
    }

    fn check_for_revert(&mut self, optb: Option<AdvanceResult>) -> Option<AdvanceResult> {
        if self.did_revert {
            self.did_revert = false;
            Some(AdvanceResult::InvalidChar)
        } else {
            optb
        }
    }

    /// simpler version of [advancex]
    pub fn advance(&mut self, input: char) -> Option<String> {
        self.advancex(input).and_then(|res| match res {
            AdvanceResult::ExpandedAfterUserdef(str) | AdvanceResult::Expanded(str) => Some(str),
            _ => None,
        })
    }

    /// removes one character from the internal buffer, or jumps back to the previous node if the
    /// buffer is empty
    pub fn revert(&mut self) {
        if self.input_buf.is_empty()
            && let Some(new_cursor_pos) = self.path.pop()
        {
            // FIXME: does not revert input_buf
            self.cur_ast_pos = new_cursor_pos;
        } else if !self.input_buf.is_empty() {
            self.input_buf.pop();
        }
    }

    fn update_cursor(&mut self, node: &FSMRc<FSMLock<FSMNode>>) {
        self.path.push(self.cur_ast_pos.clone());
        self.cur_ast_pos = FSMRc::downgrade(&FSMRc::clone(&node));
        if let NodeType::Keyword(Keyword {
            closing_token: Some(_),
            ..
        }) = &node.borrow().value
        {
            self.unfinished_nodes.push(FSMRc::downgrade(&node));
        } else if node.borrow().children.is_empty() && self.unfinished_nodes.len() > 1 {
            // we don't need to jump back if only one remains
            self.cur_ast_pos = self.unfinished_nodes.pop().unwrap();
        }
        debug_println!(
            "uc: {:?} {}/{:?}",
            self.get_cur_ast_binding().borrow().value,
            self.get_cur_ast_binding().borrow().id(),
            node.borrow().value
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
            UserDefinedRegex(..) | UserDefined { .. } if !binding.is_done() => false,
            _ => binding.children.is_empty() || !binding.has_useful_children(),
        }
    }

    fn get_cur_ast_binding(&self) -> FSMRc<FSMLock<FSMNode>> {
        self.cur_ast_pos.upgrade().unwrap()
    }
    pub fn is_in_userdefined_stage(&self) -> bool {
        match self.get_cur_ast_binding().borrow().value {
            NodeType::UserDefined { .. }
            | NodeType::UserDefinedRegex(..)
            | NodeType::UserDefinedCombo(_, _) => true,
            _ => false,
        }
    }

    fn get_current_nodeval(&self) -> NodeType {
        println!("{}", self.get_cur_ast_binding().borrow().id());
        self.get_cur_ast_binding().borrow().value.clone()
    }
    fn find_node_with_code(&self, short: &str) -> Option<FSMRc<FSMLock<FSMNode>>> {
        let binding = self.get_cur_ast_binding();
        let binding = binding.borrow();
        binding.find_node_with_code(short)
    }
}

#[cfg(test)]
mod tests {
    use crate::frontend::create_graph_from_ebnf;

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
        select ::= 'SELECT' '*' | collist 'FROM' #'^.*$' ';';
        insert ::= 'INSERT INTO' #'^.*$' ' ' 'VALUES' '(' collist ')';
        collist ::= col ( ',' collist )?;
        col ::= #'^.*[, ]$';
    ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        root.borrow().dbg();
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

    fn util_check_str(root: &FSMRc<FSMLock<FSMNode>>, str: &str) {
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
    fn test_userdefs() {
        let bnf = r"
        t1 ::= ( #'[0-9]' 't' ) | ( #'[a-z]' 'e' );
    ";
        let root = frontend::create_graph_from_ebnf(bnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!(None, cursor.advance('1'));
        assert_eq!("t", cursor.advance('t').unwrap());
        assert!(cursor.is_done());

        let mut cursor = FSMCursor::new(&root);
        assert_eq!(None, cursor.advance('a'));
        assert_eq!("e", cursor.advance('e').unwrap());
        assert!(cursor.is_done());
    }
    #[test]
    fn test_minify() {
        let root = FSMNode::new_null(None);
        let child = FSMNode::new_null(Some(&root));
        let child = FSMNode::new_keyword_with_parent("asdf".to_string(), child);
        // minify
        root.borrow().dbg();
        FSMNode::minify(&root);
        root.borrow().dbg();
        assert_eq!(Null, root.borrow().value);
        assert_eq!(
            Keyword(Keyword::new("asdf".to_string(), None)),
            root.borrow().children[0].borrow().value
        );
        assert_eq!(1, root.borrow().children.len())
    }

    #[test]
    fn test_minify_multiple() {
        let root = FSMNode::new_null(None);
        let child = FSMNode::new_null(Some(&root));
        let child = FSMNode::new_null(Some(&child));
        let child = FSMNode::new_keyword_with_parent("asdf".to_string(), child);
        // minify
        root.borrow().dbg();
        FSMNode::minify(&root);
        root.borrow().dbg();
        assert_eq!(Null, root.borrow().value);
        assert_eq!(
            Keyword(Keyword::new("asdf".to_string(), None)),
            root.borrow().children[0].borrow().value
        );
        assert_eq!(1, root.borrow().children.len())
    }

    #[test]
    fn test_minify_cycles() {
        let root = FSMNode::new_null(None);
        let child = FSMNode::new_null(Some(&root));
        let child2 = FSMNode::new_null(Some(&child));
        let child = FSMNode::new_keyword_with_parent("asdf".to_string(), child2.clone());
        FSMNode::add_child_cycle_safe(&child, &child2);
        FSMNode::minify(&root);

        assert_eq!(Null, root.borrow().value);
        assert_eq!(
            Keyword(Keyword::new("asdf".to_string(), None)),
            root.borrow().children[0].borrow().value
        );
        assert_eq!(1, root.borrow().children.len());
        assert_eq!(1, root.borrow().children[0].borrow().children.len());
    }

    #[test]
    fn test_userdef_links() {
        let root = create_graph_from_ebnf(
            r"
     main ::= #'[0-9]+' ( ('-' 'test') | (';' 'uwu') );
",
        )
        .unwrap();
        let mut cursor = FSMCursor::new(&root);
        for i in 0..=9 {
            assert_eq!(None, cursor.advance(i.to_string().chars().nth(0).unwrap()));
        }
        let mut cursor2 = cursor.clone();
        assert_eq!("-", cursor.advance('-').unwrap());
        assert_eq!("test", cursor.advance('t').unwrap());
        assert_eq!(";", cursor2.advance(';').unwrap());
        assert_eq!("uwu", cursor2.advance('u').unwrap());
        root.borrow().dbg();
    }

    #[test]
    fn test_fancy_regex_usecase() {
        let root = create_graph_from_ebnf(
            r"
     main ::= (#'[0-9]+' 'uwu') | (#'[a-z]' 'awa');
",
        )
        .unwrap();
        let mut cursor = FSMCursor::new(&root);
        let mut cursor2 = cursor.clone();
        assert_eq!(None, cursor.advance('0'));
        assert_eq!("uwu", cursor.advance('u').unwrap());
        assert_eq!(None, cursor2.advance('b'));
        assert_eq!("awa", cursor2.advance('a').unwrap());
        root.borrow().dbg();
    }

    #[test]
    fn test_reset() {
        let root = FSMNode::new_null(None);
        let other = FSMNode::new_keyword_with_parent("int".to_string(), root.clone());
        let _other = FSMNode::new_keyword_with_parent("asdf".to_string(), other.clone());
        assert_eq!(root.borrow().children.len(), 1);
        let mut cursor = FSMCursor::new(&root);
        assert_eq!("int", cursor.advance('i').unwrap());
        assert_eq!(None, cursor.advance('i'));
        cursor.reset();
        assert_eq!("int", cursor.advance('i').unwrap());
    }

    #[test]
    fn test_revert() {
        let root = FSMNode::new_null(None);
        let other = FSMNode::new_keyword_with_parent("int".to_string(), root.clone());
        let _other = FSMNode::new_keyword_with_parent("asdf".to_string(), other.clone());
        let mut cursor = FSMCursor::new(&root);

        assert_eq!("int", cursor.advance('i').unwrap());
        assert_eq!("asdf", cursor.advance('a').unwrap());
        cursor.revert();
        assert_eq!("asdf", cursor.advance('a').unwrap());
    }

    #[test]
    fn test_dead_end_prevention() {
        let root = FSMNode::new_null(None);
        let other = FSMNode::new_keyword_with_parent("int".to_string(), root.clone());
        let _other = FSMNode::new_keyword_with_parent("asdf".to_string(), other.clone());
        let mut cursor = FSMCursor::new(&root);

        assert_eq!("int", cursor.advance('i').unwrap());
        assert_eq!(None, cursor.advance('i'));
        assert_eq!(None, cursor.advance('?'));
        assert_eq!("asdf", cursor.advance('a').unwrap());
    }
    #[test]
    fn test_dead_end_prevention_userdef() {
        let ebnf = r"
        t1 ::= ( #'[0-9]' 'asdf' ) | ( #'[a-z]' 'test' );
        ";
        let root = create_graph_from_ebnf(ebnf).unwrap();
        let mut cursor = FSMCursor::new(&root);

        assert_eq!(None, cursor.advance('0'));
        assert_eq!("asdf", cursor.advance('a').unwrap());
        cursor.reset();
        assert_eq!(None, cursor.advance('a'));
        assert_eq!("test", cursor.advance('t').unwrap());
        cursor.reset();
        assert_eq!(None, cursor.advance('A'));
        assert_eq!(None, cursor.advance('B'));
        assert_eq!(None, cursor.advance('!'));

        assert_eq!(None, cursor.advance('a'));
        assert_eq!("test", cursor.advance('t').unwrap());
    }
    #[test]
    fn test_advancex() {
        let ebnf = r"
        t1 ::= ( #'[0-9]' 'asdf' ) | ( 'test' );
        ";
        let root = create_graph_from_ebnf(ebnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!(None, cursor.advancex('0'));
        assert_eq!(
            AdvanceResult::ExpandedAfterUserdef("asdf".to_string()),
            cursor.advancex('a').unwrap()
        );
        cursor.reset();
        assert_eq!(AdvanceResult::InvalidChar, cursor.advancex('!').unwrap());
        assert_eq!(
            AdvanceResult::Expanded("test".to_string()),
            cursor.advancex('t').unwrap()
        );
    }

    #[test]
    fn dead_end_edgecase() {
        let ebnf = r"
        t1 ::= #'[0-9]' '=';
        ";
        let root = create_graph_from_ebnf(ebnf).unwrap();
        let mut cursor = FSMCursor::new(&root);
        assert_eq!(None, cursor.advance('0'));
        assert_eq!("=", cursor.advance('=').unwrap());
    }
}
