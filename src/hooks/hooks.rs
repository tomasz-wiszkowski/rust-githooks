use std::collections::HashMap;

use crate::repo::config::GitConfigManager;
use super::hook::Hook;
use std::cell::RefCell;
use super::config::load_config_file;

// A map of all known and user-defined hooks and their corresponding actions.
// The key is the hook name, and the value is the corresponding Hook definition.
pub type Hooks = HashMap<String, Hook>;

static mut K_KNOWN_HOOKS: Option<RefCell<Hooks>> = None;

// Retrieve the map of user-defined hooks.
// Upon first call the function will attempt to load user-defined hooks from
// the ~/.githooks.json config file.
pub fn get_hooks() -> &'static RefCell<Hooks> {
    unsafe {
        if K_KNOWN_HOOKS.is_none() {
            K_KNOWN_HOOKS = Some(load_config_file().unwrap_or_default());
        }
        K_KNOWN_HOOKS.as_ref().unwrap()
    }
}

// Specify the configuration store persisting action configuration relevant to
// the current context (typically the current git repository).
pub trait HooksExt {
    fn set_config_store(&mut self, s: &GitConfigManager);
}

impl HooksExt for Hooks {
    fn set_config_store(&mut self, s: &GitConfigManager) {
        for (_, mut hook) in self.iter_mut() {
            hook.set_config_store(s);
        }
    }
}

