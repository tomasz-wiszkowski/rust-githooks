use anyhow::Result;
use std::collections::HashMap;

use super::config::load_config_file;
use super::hook::Hook;
use crate::repo::config::GitConfigManager;

// A map of all known and user-defined hooks and their corresponding actions.
// The key is the hook name, and the value is the corresponding Hook definition.
pub type Hooks = HashMap<String, Hook>;

// Retrieve the map of user-defined hooks.
// Upon first call the function will attempt to load user-defined hooks from
// the ~/.githooks.json config file.
pub fn get_hooks() -> Hooks {
    let mut hooks = load_config_file().unwrap();
    hooks.iter_mut().for_each(|(_, h)| h.sort_actions());
    hooks
}

// Specify the configuration store persisting action configuration relevant to
// the current context (typically the current git repository).
pub trait HooksExt {
    fn set_config_store(&mut self, s: &GitConfigManager) -> Result<()>;
}

impl HooksExt for Hooks {
    fn set_config_store(&mut self, s: &GitConfigManager) -> Result<()> {
        for (_, hook) in self.iter_mut() {
            hook.set_config_store(s)?;
        }
        Ok(())
    }
}
