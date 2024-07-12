use crate::repo::*;
use anyhow::{bail, ensure, Result};
use regex::Regex;
use serde_derive::Deserialize;
use std::path::Path;

use super::shell_utils;

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
    #[cfg(test)]
    pub fn new_for_test(name: &str) -> Self {
        Self {
            name: name.into(),
            priority: 0,
            file_pattern: Regex::new(".*").unwrap(),
            shell_cmd: vec![],
            run_type: RUN_TYPE_PER_FILE.into(),

            selected: false,
            available: true,
            config: None,
        }
    }

    pub fn set_shell_cmd(&mut self, cmd: &str) -> Result<()> {
        if let Some(command) = shell_utils::get_shell_command_absolute_path(cmd) {
            if let Some(str_command) = command.to_str().map(|s| s.to_owned()) {
                self.shell_cmd[0] = str_command;
                self.available = true;
            }
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn priority(&self) -> i32 {
        self.priority
    }

    pub fn run(&self, files: &[String], args: &Vec<String>) -> Result<()> {
        if !self.is_selected() {
            return Ok(());
        }
        if !self.is_available() {
            println!(
                "Cannot run {} - missing command {}",
                self.name(),
                self.shell_cmd[0]
            );
            return Ok(());
        }

        let mut substitutions = std::collections::HashMap::new();
        substitutions.insert(
            PLACEHOLDER_GIT_ARGS.to_owned(),
            shell_utils::Substitution::Array(args),
        );

        for file in files {
            let base = Path::new(file).file_name().unwrap().to_str().unwrap();

            if !self.file_pattern.is_match(base) {
                continue;
            }

            substitutions.insert(
                PLACEHOLDER_SINGLE_FILE.to_owned(),
                shell_utils::Substitution::Scalar(file.clone()),
            );
            let cmd = shell_utils::substitute_command_line(&self.shell_cmd, &substitutions);

            match self.run_type.as_str() {
                RUN_TYPE_PER_COMMIT => println!("Running {}", self.name),
                RUN_TYPE_PER_FILE => println!("Running {} on {}", self.name, file),
                _ => anyhow::bail!("Invalid runType {} for action {}", self.run_type, self.name),
            }

            shell_utils::run_shell_command(&cmd)?;
            if self.run_type == RUN_TYPE_PER_COMMIT {
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn check_valid(&self) -> Result<()> {
        ensure!(!self.name.is_empty(), "Hook name not set");
        ensure!(!self.shell_cmd.is_empty(), "Shell command not set");

        Ok(())
    }

    pub fn set_selected(&mut self, want_selected: bool) -> Result<()> {
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

    pub fn set_config(&mut self, cfg: Box<dyn GitConfig>) -> Result<()> {
        let selected = cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE;
        let command = cfg.get_or_default(KEY_COMMAND, &self.shell_cmd[0]);
        self.config = Some(cfg);
        self.set_selected(selected)?;
        self.set_shell_cmd(&command)?;
        Ok(())
    }
}
