use crate::repo::GitConfig;
use crate::repo::Item;
use anyhow::{bail, ensure, Result};
use log::info;
use log::warn;
use regex::Regex;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use super::shell_utils;
use super::shell_utils::Substitution;
use super::ActionTrait;

const KEY_ENABLED: &str = "enabled";
const KEY_COMMAND: &str = "cmd";
const VALUE_TRUE: &str = "true";
const PLACEHOLDER_SINGLE_FILE: &str = "<file>";
const PLACEHOLDER_GIT_ARGS: &str = "<args>";
const RUN_TYPE_PER_FILE: &str = "perFile";
const RUN_TYPE_PER_COMMIT: &str = "perCommit";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellAction {
    name: String,
    priority: i32,
    #[serde(with = "serde_regex")]
    file_pattern: Regex,
    shell_cmd: Vec<String>,
    run_type: String,

    #[serde(skip_deserializing)]
    selected: bool,
    #[serde(skip_deserializing)]
    available: bool,
    #[serde(skip_deserializing)]
    config: Option<Box<dyn GitConfig>>,
}

impl ShellAction {
    pub fn set_shell_cmd(&mut self, cmd: &str) {
        if let Some(command) = shell_utils::get_shell_command_absolute_path(cmd) {
            if let Some(str_command) = command.to_str().map(|s| s.to_owned()) {
                self.shell_cmd[0] = str_command;
                self.available = true;
            }
        }
    }

    fn run_for_commit(&self, substitutions: HashMap<String, Substitution>) -> Result<()> {
        info!("Running {}", self.name);
        let cmd = shell_utils::substitute_command_line(&self.shell_cmd, &substitutions);
        shell_utils::run_shell_command(&cmd)?;
        Ok(())
    }

    fn run_for_each_file(
        &self,
        mut substitutions: HashMap<String, Substitution>,
        files: &[&str],
    ) -> Result<()> {
        files
            .into_iter()
            .map(|&file| {
                substitutions.insert(
                    PLACEHOLDER_SINGLE_FILE.to_owned(),
                    Substitution::Scalar(file.to_owned()),
                );

                info!("Running {} on {}", self.name, file);
                shell_utils::run_shell_command(&shell_utils::substitute_command_line(
                    &self.shell_cmd,
                    &substitutions,
                ))?;
                Ok(())
            })
            .collect::<Result<_>>()
    }
}

impl ActionTrait for ShellAction {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn run(&self, items: &[Item], args: &Vec<String>) -> Result<()> {
        if !self.is_selected() {
            return Ok(());
        }
        if !self.is_available() {
            warn!(
                "Cannot run {} - missing command {}",
                self.name(),
                self.shell_cmd[0]
            );
            return Ok(());
        }

        let substitutions =
            HashMap::from([(PLACEHOLDER_GIT_ARGS.to_owned(), Substitution::Array(args))]);

        let files = items
            .iter()
            .filter_map(|f| {
                // Pick only files.
                let Item::File(name) = f else {
                    return None;
                };

                // Pick only files that match pattern.
                let just_file_name = Path::new(name).file_name().unwrap().to_str().unwrap();
                if !self.file_pattern.is_match(just_file_name) {
                    return None;
                }

                Some(name.as_str())
            })
            .collect::<Vec<_>>();

        if files.is_empty() {
            return Ok(());
        }

        match self.run_type.as_str() {
            RUN_TYPE_PER_FILE => self.run_for_each_file(substitutions, &files),
            RUN_TYPE_PER_COMMIT => self.run_for_commit(substitutions),
            _ => anyhow::bail!("Invalid runType {} for action {}", self.run_type, self.name),
        }
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn set_selected(&mut self, want_selected: bool) -> Result<()> {
        self.selected = want_selected;

        let Some(cfg) = &mut self.config else {
            bail!("Config store not available");
        };

        if want_selected {
            cfg.set(KEY_ENABLED, VALUE_TRUE)
        } else {
            cfg.remove(KEY_ENABLED)
        }
    }
}

impl super::ActionTraitInternal for ShellAction {
    fn set_config(&mut self, cfg: Box<dyn GitConfig>) -> Result<()> {
        self.selected = cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE;
        self.set_shell_cmd(&cfg.get_or_default(KEY_COMMAND, &self.shell_cmd[0]));
        self.config = Some(cfg);
        Ok(())
    }

    fn check_valid(&self) -> Result<()> {
        ensure!(!self.name.is_empty(), "Hook name not set");
        ensure!(!self.shell_cmd.is_empty(), "Shell command not set");

        Ok(())
    }
}
