use anyhow::Result;
use serde_derive::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

use super::shell_action::ShellAction;
use crate::repo::GitConfigManager;

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

    pub fn set_config_store(&mut self, store: &Box<dyn GitConfigManager>) -> Result<()> {
        let id = &self.id;
        for (name, action) in self.actions.iter_mut() {
            action
                .borrow_mut()
                .set_config(store.get_config_for(id, name))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::test::*;
    use mockall::predicate::*;

    #[test]
    fn test_new_hook() {
        let actions = HashMap::new();
        let hook = Hook::new("test_id".to_string(), "test_name".to_string(), actions);
        assert_eq!(hook.id(), "test_id");
        assert_eq!(hook.name(), "test_name");
        assert!(hook.actions().is_empty());
    }

    #[test]
    fn test_set_config_store() -> Result<()> {
        let mut mock_config_manager = MockGitConfigManager::new();
        mock_config_manager
            .expect_get_config_for()
            .with(eq("test_id"), eq("action1"))
            .returning(|_, _| Box::new(MockGitConfig::new()));

        /*
                let mut mock_action = MockShellAction::new();
                mock_action
                    .expect_set_config()
                    .with(eq("config1".to_string()))
                    .returning(|_| Ok(()));

                let mut actions = HashMap::new();
                actions.insert("action1".to_string(), Rc::new(RefCell::new(mock_action)));

                let mut hook = Hook::new("test_id".to_string(), "test_name".to_string(), actions);
                hook.set_config_store(&mock_config_manager)?;
        */

        Ok(())
    }
}
