use anyhow::{bail, Context, Result};
use serde_derive::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

use super::hook::Hook;
use super::shell_action::ShellAction;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookConfig {
    name: String,
    actions: HashMap<String, Rc<RefCell<ShellAction>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TopConfig {
    version: i32,
    hooks: HashMap<String, HookConfig>,
}

pub fn load_config_file() -> Result<HashMap<String, Hook>> {
    let home_dir = dirs::home_dir().context("Unable to query user home directory")?;
    let config_path = home_dir.join(".githooks.json");

    let content = fs::read_to_string(&config_path)?;

    let config: TopConfig = serde_json::from_str(&content).context("Malformed config file")?;

    if config.version == 0 {
        bail!("Unsupported config file version");
    }

    anyhow::ensure!(
        config.version == 1,
        "Unsupported config file version {}",
        config.version
    );

    let mut result = HashMap::new();

    for (ck, cv) in config.hooks {
        anyhow::ensure!(!ck.is_empty(), "Invalid category ID");
        anyhow::ensure!(
            !cv.name.is_empty(),
            "Invalid category name for category {}",
            ck
        );

        for (hk, hv) in cv.actions.iter() {
            anyhow::ensure!(!hk.is_empty(), "Invalid hook ID in category {}", ck);
            hv.borrow()
                .check_valid()
                .context(format!("while evaluating {}/{}", ck, hk))?;
        }

        let category = Hook::new(ck.clone(), cv.name, cv.actions);

        result.insert(ck, category);
    }

    Ok(result)
}
