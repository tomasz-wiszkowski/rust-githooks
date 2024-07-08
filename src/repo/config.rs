use anyhow::Result;
use git2::{Repository, Config};
use std::error::Error;
use std::rc::Rc;
use std::cell::RefCell;

pub struct GitConfigManager {
    repo: Rc<Repository>,
    config: Rc<RefCell<Config>>,
}

pub struct GitConfig {
    config: Rc<RefCell<Config>>,
    section: String,
    hook: String,
}

impl GitConfigManager {
    pub fn new(repo: &Rc<Repository>) -> Result<Self> {
        Ok(GitConfigManager {
            repo: repo.clone(),
            config: Rc::new(RefCell::new(repo.config()?))
        })
    }

    pub fn get_config_for(&self, category_id: &str, hook_id: &str) -> GitConfig {
        GitConfig {
            config: self.config.clone(),
            section: category_id.to_string(),
            hook: hook_id.to_string(),
        }
    }
}

impl GitConfig {
    pub fn has(&self, key: &str) -> bool {
        let full_key = format!("{}.{}.{}", self.section, self.hook, key);
        self.config.borrow().get_entry(&full_key).is_ok()
    }

    pub fn get_or_default(&self, key: &str, default: &str) -> String {
        let full_key = format!("{}.{}.{}", self.section, self.hook, key);
        self.config.borrow().get_string(&full_key).unwrap_or(default.to_string())
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), Box<dyn Error>> {
        let full_key = format!("{}.{}.{}", self.section, self.hook, key);
        self.config.borrow_mut().set_str(&full_key, value)?;
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Result<(), Box<dyn Error>> {
        let full_key = format!("{}.{}.{}", self.section, self.hook, key);
        self.config.borrow_mut().remove(&full_key)?;
        Ok(())
    }
}

