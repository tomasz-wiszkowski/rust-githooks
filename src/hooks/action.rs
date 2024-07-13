use crate::repo::GitConfig;
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;

use std::collections::HashMap;

pub trait ActionTraitInternal {
    fn set_config(&mut self, cfg: Box<dyn GitConfig>) -> Result<()>;
    fn check_valid(&self) -> Result<()>;
}

pub trait ActionTrait: ActionTraitInternal {
    fn name(&self) -> &str;
    fn priority(&self) -> i32;
    fn run(&self, files: &[String], args: &Vec<String>) -> Result<()>;
    fn is_selected(&self) -> bool;
    fn is_available(&self) -> bool;
    fn set_selected(&mut self, want_selected: bool) -> Result<()>;
}

pub type Action = Rc<RefCell<dyn ActionTrait + 'static>>;
pub type Actions = HashMap<String, Action>;
