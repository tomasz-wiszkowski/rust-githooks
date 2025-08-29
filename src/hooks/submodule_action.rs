use anyhow::bail;
use anyhow::Result;

use log::warn;
use serde_derive::Deserialize;

use crate::repo::GitConfig;
use crate::repo::Item;

use super::action::ActionTraitInternal;
use super::ActionTrait;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleAction {
    #[serde(skip_deserializing)]
    enabled: bool,
    #[serde(skip_deserializing)]
    config: Option<Box<dyn GitConfig>>,
}

const KEY_ENABLED: &str = "enabled";
const VALUE_TRUE: &str = "true";

impl ActionTraitInternal for SubmoduleAction {
    fn check_valid(&self) -> Result<()> {
        Ok(())
    }

    fn set_config(&mut self, cfg: Box<dyn crate::repo::GitConfig>) -> Result<()> {
        self.enabled = cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE;
        self.config = Some(cfg);
        Ok(())
    }
}

impl ActionTrait for SubmoduleAction {
    fn is_available(&self) -> bool {
        true
    }

    fn set_selected(&mut self, want_selected: bool) -> Result<()> {
        let Some(cfg) = self.config.as_mut() else {
            warn!("Config store not available");
            return Ok(());
        };

        if want_selected {
            cfg.set(KEY_ENABLED, VALUE_TRUE)?;
        } else {
            cfg.remove(KEY_ENABLED)?;
        }
        self.enabled = want_selected;
        Ok(())
    }

    fn is_selected(&self) -> bool {
        self.enabled
    }

    fn name(&self) -> &str {
        "Detect and Stop if Submodules are changed"
    }

    fn priority(&self) -> i32 {
        100
    }

    fn run(&self, items: &[Item], _args: &Vec<String>) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        items
            .into_iter()
            .map(|i| {
                if let Item::Commit(name) = i {
                    bail!("Attempting to include submodule {}. Aborting", name);
                }
                Ok(())
            })
            .collect::<Result<_>>()
    }
}
