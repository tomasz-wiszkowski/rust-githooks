use anyhow::{Context, Result};
use serde_derive::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

use crate::hooks::Actions;

use super::hook::Hook;
use super::Action;
use super::ActionTraitInternal;
use super::GerritChangeIdAction;
use super::ShellAction;

#[derive(Deserialize)]
#[serde(tag = "version", rename_all = "camelCase")]
enum TopConfig {
    #[serde(rename = "1")]
    V1(V1Config),

    #[serde(rename = "2")]
    V2(V2Config),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V1Config {
    hooks: HashMap<String, V1HookConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V1HookConfig {
    name: String,
    actions: HashMap<String, ShellAction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V2Config {
    hooks: HashMap<String, V2HookConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V2HookConfig {
    name: String,
    actions: HashMap<String, V2ActionConfig>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum V2ActionConfig {
    Shell(ShellAction),
    Gerrit(GerritChangeIdAction),
}

pub fn load_config_file() -> Result<HashMap<String, Hook>> {
    let home_dir = dirs::home_dir().context("Unable to query user home directory")?;
    let config_path = home_dir.join(".githooks.json");

    let content = fs::read_to_string(&config_path)?;

    let config: TopConfig = serde_json::from_str(&content).context("Malformed config file")?;

    match config {
        TopConfig::V1(cfg) => from_v1_config(cfg),
        TopConfig::V2(cfg) => from_v2_config(cfg),
    }
}

fn from_v1_config(config: V1Config) -> Result<HashMap<String, Hook>> {
    let mut result = HashMap::new();

    for (ck, mut cv) in config.hooks.into_iter() {
        anyhow::ensure!(!ck.is_empty(), "Invalid category ID");
        anyhow::ensure!(
            !cv.name.is_empty(),
            "Invalid category name for category {}",
            ck
        );

        for (hk, hv) in cv.actions.iter_mut() {
            anyhow::ensure!(!hk.is_empty(), "Invalid hook ID in category {}", ck);
            hv.check_valid()
                .context(format!("while evaluating {}/{}", ck, hk))?;
        }

        let category = Hook::new(
            ck.clone(),
            cv.name,
            cv.actions
                .into_iter()
                .map(|(n, a)| (n, Rc::new(RefCell::new(a)) as Action))
                .collect(),
        );

        result.insert(ck, category);
    }

    Ok(result)
}

fn from_v2_config(config: V2Config) -> Result<HashMap<String, Hook>> {
    let mut result = HashMap::new();

    for (ck, cv) in config.hooks.into_iter() {
        anyhow::ensure!(!ck.is_empty(), "Invalid category ID");
        anyhow::ensure!(
            !cv.name.is_empty(),
            "Invalid category name for category {}",
            ck
        );

        let mut actions = Actions::new();

        for (hk, hv) in cv.actions.into_iter() {
            let action: Action = match hv {
                V2ActionConfig::Shell(a) => Rc::new(RefCell::new(a)),
                V2ActionConfig::Gerrit(a) => Rc::new(RefCell::new(a)),
            };

            anyhow::ensure!(!hk.is_empty(), "Invalid hook ID in category {}", ck);
            action
                .borrow()
                .check_valid()
                .context(format!("while evaluating {}/{}", ck, hk))?;

            actions.insert(hk, action);
        }

        let category = Hook::new(ck.clone(), cv.name, actions);

        result.insert(ck, category);
    }

    Ok(result)
}
