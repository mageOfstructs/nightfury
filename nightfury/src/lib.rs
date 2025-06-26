#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(trait_alias)]
#![feature(impl_trait_in_bindings)]
#![feature(lock_value_accessors)]
#![feature(pattern)]

use debug_print::debug_println;
use fsm::NodeType::{self, *};
use fsm::{CycleAwareOp, Keyword};
pub use fsm::{FSMNode, ToCSV};
use regex::Regex;
#[cfg(not(feature = "thread-safe"))]
use std::cell::{Ref, RefCell, RefMut};
#[cfg(feature = "thread-safe")]
use std::sync::Mutex;
#[cfg(feature = "thread-safe")]
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub mod frontend;

mod fsm;
pub use fsm::FSMNodeWrapper;

pub mod protocol;

#[cfg(not(feature = "thread-safe"))]
thread_local! {
    static CNT: RefCell<usize> = RefCell::new(0);
}

#[cfg(feature = "thread-safe")]
static CNT: Mutex<usize> = Mutex::new(0);
fn get_id() -> usize {
    let mut ret = 0;
    #[cfg(not(feature = "thread-safe"))]
    {
        CNT.with_borrow(|cnt| ret = *cnt);
        CNT.with_borrow_mut(|cnt| *cnt += 1);
    }
    #[cfg(feature = "thread-safe")]
    {
        let mut lock = CNT.lock().expect("CNT lock");
        ret = *lock;
        *lock += 1;
    }
    return ret;
}
fn dbg_id() {
    #[cfg(feature = "thread-safe")]
    debug_println!("{:?}", CNT.lock());
    #[cfg(not(feature = "thread-safe"))]
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
                return true;
            }
        }
        false
    }
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
        self.0.write().expect("FSMLock borrow()")
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
#[derive(Clone, Debug)]
pub struct FSMCursor {
    root: InternalCursor,
    cur_ast_pos: InternalCursor,
    input_buf: String,
    unfinished_nodes: Vec<InternalCursor>,
}

impl FSMCursor {
    pub fn new(fsm_root: &FSMRc<FSMLock<FSMNode>>) -> Self {
        Self {
            root: FSMRc::downgrade(fsm_root),
            cur_ast_pos: FSMRc::downgrade(fsm_root),
            input_buf: String::new(),
            unfinished_nodes: Vec::new(),
        }
    }
    pub fn reset(&mut self) {
        self.cur_ast_pos = FSMWeak::clone(&self.root);
        self.input_buf.clear();
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
                    if regex.partial_match(&self.input_buf) =>
                {
                    true
                }
                _ => false,
            },
            false,
        )
    }
    pub fn search_rec(
        &self,
        treenode: &FSMRc<FSMLock<FSMNode>>,
    ) -> Option<FSMRc<FSMLock<FSMNode>>> {
        self.search_rec_internal(treenode, false)
    }
    pub fn search_rec_internal(
        &self,
        treenode: &FSMRc<FSMLock<FSMNode>>,
        best_effort: bool,
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
        // TODO: this one relies on buggy behavior from the old function!
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
        if borrow.is_done() {
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
                    binding.borrow_mut().set_is_done(true);
                    self.input_buf.clear();
                    self.input_buf.push(input);
                    let next_node = self.search_rec_internal(&binding, true);
                    if next_node.is_none() {
                        debug_println!("No node found");
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
            UserDefinedCombo(_, f) => {
                let ret = self.handle_userdefined_combo(input, f);
                if ret.is_some() {
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
                        NodeType::UserDefinedCombo(r, f) => {
                            let res = self.handle_userdefined_combo(input, f);
                            res
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
        None
    }

    fn update_cursor(&mut self, node: &FSMRc<FSMLock<FSMNode>>) {
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
            "uc: {:?} {}",
            self.get_cur_ast_binding().borrow().value,
            self.get_cur_ast_binding().borrow().id()
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
}
