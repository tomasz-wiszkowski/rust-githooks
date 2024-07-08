use anyhow::Result;

use crate::repo::config::GitConfig;

pub trait Action {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn priority(&self) -> i32;
    fn set_selected(&mut self, selected: bool);
    fn is_selected(&self) -> bool;
    fn is_available(&self) -> bool;
    fn set_config(&mut self, config: GitConfig);
    fn run(&self, file: &[String], args: &[String]) -> Result<()>;
}

pub struct Config;

