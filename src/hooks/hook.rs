use anyhow::Result;
use serde_derive::Deserialize;

use super::shell_action::ShellAction;
use crate::repo::config::GitConfigManager;

#[derive(Deserialize)]
// #[serde(tag = "type", rename_all = "camelCase")]
pub struct Hook {
    id: String,
    name: String,
    actions: Vec<ShellAction>,
}

impl Hook {
    pub fn new(id: String, name: String, actions: Vec<ShellAction>) -> Self {
        Self { id, name, actions }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn actions(&self) -> &[ShellAction] {
        &self.actions
    }

    pub fn actions_mut(&mut self) -> &mut [ShellAction] {
        &mut self.actions
    }

    pub fn set_config_store(&mut self, store: &GitConfigManager) -> Result<()> {
        let id = &self.id;
        for action in self.actions.iter_mut() {
            action.set_config(store.get_config_for(id, action.id()))?;
        }
        Ok(())
    }

    pub fn sort_actions(&mut self) {
        self.actions.sort_by_key(|a| a.priority());
    }
}
