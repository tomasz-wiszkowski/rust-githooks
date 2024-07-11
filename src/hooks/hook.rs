use anyhow::Result;
use serde_derive::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

use super::shell_action::ShellAction;
use crate::repo::config::GitConfigManager;

use std::collections::HashMap;

pub type Action = Rc<RefCell<ShellAction>>;
pub type Actions = HashMap<String, Action>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hook {
    id: String,
    name: String,
    actions: Actions,
}

impl Hook {
    pub fn new(id: String, name: String, actions: Actions) -> Self {
        Self { id, name, actions }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn actions(&self) -> &Actions {
        &self.actions
    }

    pub fn set_config_store(&mut self, store: &GitConfigManager) -> Result<()> {
        let id = &self.id;
        for (name, action) in self.actions.iter_mut() {
            action
                .borrow_mut()
                .set_config(store.get_config_for(id, name))?;
        }
        Ok(())
    }
}
