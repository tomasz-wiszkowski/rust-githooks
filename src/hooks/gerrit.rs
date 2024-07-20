use anyhow::bail;
use anyhow::Context;
use anyhow::Result;

use chrono::Local;
use git2::Config;
use git2::Repository;
use log::info;
use log::warn;
use regex::Regex;
use std::collections::BTreeMap;
use std::env;
use std::fs;

use crate::repo::GitConfig;

use super::action::ActionTraitInternal;
use super::ActionTrait;

pub struct GerritChangeIdAction {
    enabled: bool,
    config: Option<Box<dyn GitConfig>>,
}

const TAG_CHANGE_ID: &'static str = "Change-Id";
const TAG_CHANGE_LINK: &'static str = "Link";
const KEY_ENABLED: &str = "enabled";
const VALUE_TRUE: &str = "true";

impl GerritChangeIdAction {
    pub fn new() -> Self {
        Self {
            enabled: false,
            config: None,
        }
    }

    fn generate_hash(repo: &Repository) -> String {
        let mut hash_data = String::new();
        if let Ok(config) = repo.config() {
            hash_data += &config.get_string("user.name").unwrap_or_default();
            hash_data += &config.get_string("user.email").unwrap_or_default();
        }

        if let Ok(head) = repo.head() {
            hash_data += head
                .target()
                .map(|t| t.to_string())
                .unwrap_or_default()
                .as_str();
        }

        hash_data += Local::now().to_rfc3339().as_str();
        let hasher = git2::Oid::hash_object(git2::ObjectType::Blob, hash_data.as_bytes()).unwrap();

        info!("Using Change ID: I{}", hasher);
        hasher.to_string()
    }

    fn should_generate_change_id(config: &Config, commit_msg: &str) -> bool {
        match config
            .get_string("gerrit.createChangeId")
            .unwrap_or_default()
            .as_str()
        {
            "false" => {
                info!("Skipping Change-Id creation: gerrit.createChangeId set to `false'");
                false
            }

            "always" => {
                info!("Forcing Change-Id creation: gerrit.createChangeId set to `always'");
                true
            }

            _ => {
                // Do not create a change id for squash/fixup commits.
                let first_line = commit_msg.lines().next().unwrap_or_default();
                let generate_change_id =
                    !Regex::new(r"^[a-z][a-z]*! ").unwrap().is_match(first_line);
                if !generate_change_id {
                    info!("Skipping squash/fixup commit: {}", first_line);
                }
                generate_change_id
            }
        }
    }

    fn generate_change_id(file_path: &str) -> Result<()> {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

        let args: Vec<String> = env::args().collect();
        if args.len() != 2 {
            bail!("{} requires an argument.", args[0]);
        }

        let file_content = fs::read_to_string(file_path)?;
        let (commit_msg, mut trailers) = Self::parse_commit_message(&file_content)?;

        let repo = Repository::open(".").expect("Failed to open repository");
        let config = repo.config().expect("Failed to get config");

        if !Self::should_generate_change_id(&config, &commit_msg) {
            return Ok(());
        }

        if let Some(review_url) = config.get_string("gerrit.reviewUrl").ok() {
            info!("Checking for `{}'", TAG_CHANGE_LINK);
            trailers.entry(TAG_CHANGE_LINK.into()).or_insert_with(|| {
                format!(
                    "{}/id/I{}",
                    review_url.trim_end_matches('/'),
                    Self::generate_hash(&repo)
                )
            });
        } else {
            info!("Checking for `{}'", TAG_CHANGE_ID);
            trailers
                .entry(TAG_CHANGE_ID.into())
                .or_insert_with(|| format!("I{}", Self::generate_hash(&repo)));
        };

        Ok(fs::write(
            &file_path,
            Self::compile_commit_message(&commit_msg, &trailers),
        )
        .context(format!("Can't save file: {}", file_path))?)
    }

    fn parse_commit_message(message: &str) -> Result<(String, BTreeMap<String, String>)> {
        let mut lines: Vec<&str> = message
            .lines()
            .take_while(|line| !line.starts_with(">8"))
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect();

        // Confirm that the second line is empty
        if lines.len() > 1 && !lines[1].is_empty() {
            bail!("Not a valid git commit message");
        }

        // Skip empty lines at end...
        while lines.len() > 2 {
            if !lines.last().unwrap().is_empty() {
                break;
            }
            lines.pop();
        }

        // ... then, process trailers...
        let mut trailers = BTreeMap::new();
        let trailer_regex = Regex::new(r"^([a-zA-Z0-9-]+):\s*(.*)\s*$").unwrap();

        while lines.len() > 2 {
            let line = lines.last().unwrap();
            if let Some(caps) = trailer_regex.captures(line) {
                info!("Found trailer {}: {}", &caps[1], &caps[2]);
                trailers.insert(caps[1].to_string(), caps[2].to_string());
                lines.pop();
            }
        }

        // ... then, skip empty lines at end.
        while lines.len() > 2 {
            if !lines.last().unwrap().is_empty() {
                break;
            }
            lines.pop();
        }

        let commit_message = lines.join("\n");
        Ok((commit_message, trailers))
    }

    fn compile_commit_message(message: &str, trailers: &BTreeMap<String, String>) -> String {
        let mut res = message.to_owned();

        res += "\n";
        for (key, val) in trailers {
            res += format!("{}: {}\n", key, val).as_str();
        }
        res
    }
}

impl ActionTraitInternal for GerritChangeIdAction {
    fn check_valid(&self) -> Result<()> {
        Ok(())
    }

    fn set_config(&mut self, cfg: Box<dyn crate::repo::GitConfig>) -> Result<()> {
        self.enabled = cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE;
        Ok(())
    }
}

impl ActionTrait for GerritChangeIdAction {
    fn is_available(&self) -> bool {
        true
    }

    fn set_selected(&mut self, want_selected: bool) -> Result<()> {
        let Some(cfg) = self.config.as_mut() else {
            warn!("Config store not available");
            return Ok(());
        };

        if want_selected {
            cfg.set(KEY_ENABLED, VALUE_TRUE)
        } else {
            cfg.remove(KEY_ENABLED)
        }
    }

    fn is_selected(&self) -> bool {
        self.enabled
    }

    fn name(&self) -> &str {
        "Gerrit ChangeId generator"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn run(&self, _files: &[String], args: &Vec<String>) -> Result<()> {
        info!("Would generate change id for {}", args[0]);
        Ok(())
        //        Self::generate_change_id(args[0])
    }
}
