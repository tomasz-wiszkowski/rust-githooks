use std::collections::HashMap;
use std::fs;
use serde_derive::Deserialize;
use anyhow::{Context, Result,bail};

use super::hook::Hook;
use super::shell_action::ShellAction;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionConfig {
    name: String,
    run_type: String,
    priority: i32,
    file_pattern: String,
    shell_cmd: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookConfig {
    name: String,
    actions: HashMap<String, ActionConfig>,
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

    let config: TopConfig = serde_json::from_str(&content)
        .context("Malformed config file")?;

    if config.version == 0 {
        bail!("Unsupported config file version");
    }

    anyhow::ensure!(config.version == 1, "Unsupported config file version {}", config.version);

    let mut result = HashMap::new();

    for (ck, cv) in config.hooks {
        anyhow::ensure!(!ck.is_empty(), "Invalid category ID");
        anyhow::ensure!(!cv.name.is_empty(), "Invalid category name for category {}", ck);

        let mut hooks = Vec::new();

        for (hk, hv) in cv.actions {
            anyhow::ensure!(!hk.is_empty(), "Invalid hook ID in category {}", ck);
            anyhow::ensure!(!hv.name.is_empty(), "Invalid hook name for hook {}", hk);
            anyhow::ensure!(!hv.shell_cmd.is_empty(), "Invalid shell command for hook {}", hk);

            let hook = ShellAction::new(
                &hk, &hv.name, hv.priority, &hv.file_pattern, hv.shell_cmd, hv.run_type
            )?;

            hooks.push(hook);
        }

        let category = Hook::new(ck.clone(), cv.name, hooks);

        result.insert(ck, category);
    }

    Ok(result)
}
