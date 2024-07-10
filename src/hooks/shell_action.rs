use anyhow::{bail, Result};
use regex::Regex;
use serde_derive::Deserialize;
use std::path::Path;

use crate::repo::config::GitConfig;

use super::shell_utils::{self, Substitution};

const KEY_ENABLED: &str = "enabled";
const KEY_COMMAND: &str = "cmd";
const VALUE_TRUE: &str = "true";
const PLACEHOLDER_SINGLE_FILE: &str = "<file>";
const PLACEHOLDER_GIT_ARGS: &str = "<args>";
const RUN_TYPE_PER_FILE: &str = "perFile";
const RUN_TYPE_PER_COMMIT: &str = "perCommit";

#[derive(Deserialize)]
pub struct ShellAction {
    id: String,
    name: String,
    priority: i32,
    #[serde(with = "serde_regex")]
    file_pattern: Regex,
    shell_command: Vec<String>,
    run_type: String,
    selected: bool,
    available: bool,

    #[serde(skip_deserializing)]
    config: Option<GitConfig>,
}

impl ShellAction {
    pub fn new(
        id: &str,
        name: &str,
        priority: i32,
        file_pattern: &str,
        shell_cmd: Vec<String>,
        run_type: String,
    ) -> Result<Self> {
        Ok(ShellAction {
            id: id.to_string(),
            name: name.to_string(),
            priority,
            file_pattern: Regex::new(file_pattern)?,
            available: false,
            shell_command: shell_cmd,
            selected: false,
            run_type,
            config: None,
        })
    }

    pub fn set_shell_cmd(&mut self, cmd: &str) -> Result<()> {
        if let Some(command) = shell_utils::get_shell_command_absolute_path(cmd) {
            if let Some(str_command) = command.to_str().map(|s| s.to_owned()) {
                self.shell_command[0] = str_command;
                self.available = true;
            }
        }
        Ok(())
    }
}

impl ShellAction {
    pub fn id(&self) -> &str {
        &self.id
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
                self.shell_command[0]
            );
            return Ok(());
        }

        let mut substitutions = std::collections::HashMap::new();
        substitutions.insert(PLACEHOLDER_GIT_ARGS.to_owned(), Substitution::Array(args));

        for file in files {
            let base = Path::new(file).file_name().unwrap().to_str().unwrap();

            if !self.file_pattern.is_match(base) {
                continue;
            }

            substitutions.insert(
                PLACEHOLDER_SINGLE_FILE.to_owned(),
                Substitution::Scalar(file.clone()),
            );
            let cmd = shell_utils::substitute_command_line(&self.shell_command, &substitutions);

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

    pub fn set_config(&mut self, cfg: GitConfig) -> Result<()> {
        self.set_selected(cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE)?;
        self.set_shell_cmd(&cfg.get_or_default(KEY_COMMAND, &self.shell_command[0]))?;
        self.config = Some(cfg);
        Ok(())
    }
}
