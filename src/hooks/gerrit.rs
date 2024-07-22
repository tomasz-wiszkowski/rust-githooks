use anyhow::bail;
use anyhow::Context;
use anyhow::Result;

use chrono::Local;

use git2::Repository;
use log::info;
use log::warn;
use regex::Regex;
use serde_derive::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;

use crate::repo::GitConfig;

use super::action::ActionTraitInternal;
use super::ActionTrait;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GerritChangeIdAction {
    #[serde(skip_deserializing)]
    enabled: bool,
    #[serde(skip_deserializing)]
    config: Option<Box<dyn GitConfig>>,
}

// const CONFIG_GERRIT_REVIEW_URL &'str = "gerrit.reviewUrl";
const TAG_CHANGE_ID: &str = "Change-Id";
const KEY_ENABLED: &str = "enabled";
const VALUE_TRUE: &str = "true";
const SKIP_CHANGE_ID_REGEX: &str = r"(^[a-z]+!|fixup[^a-zA-Z0-9_]?)";

impl GerritChangeIdAction {
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

    pub fn should_generate_change_id(
        requested_type: &str,
        commit_msg: &str,
        trailers: &BTreeMap<String, String>,
    ) -> bool {
        match requested_type {
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
                let mut skip_tag_creation = Regex::new(SKIP_CHANGE_ID_REGEX)
                    .unwrap()
                    .is_match(first_line);

                if skip_tag_creation {
                    info!("Skipping possible squash/fixup commit: {}", first_line);
                }

                skip_tag_creation |= trailers.contains_key(TAG_CHANGE_ID);
                !skip_tag_creation
            }
        }
    }

    fn generate_change_id(file_path: &str) -> Result<()> {
        let args: Vec<String> = env::args().collect();
        if args.len() != 2 {
            bail!("{} requires an argument.", args[0]);
        }

        let file_content = fs::read_to_string(file_path)?;
        let (commit_msg, mut trailers) = Self::parse_commit_message(&file_content)?;

        let repo = Repository::open(".").expect("Failed to open repository");
        let config = repo.config().expect("Failed to get config");

        if !Self::should_generate_change_id(
            &config
                .get_string("gerrit.createChangeId")
                .unwrap_or_default(),
            &commit_msg,
            &trailers,
        ) {
            return Ok(());
        }

        // Note: certain Gerrit instances use review url to dictate the change ID.
        // This may or may not work, depending on whether Gerrit instance can parse back the URL.
        // To enable this alternative behavior, use `gerrit.reviewUrl` Git config value, and
        // insert the `Link: <review_url>/id/I<change-id>` trailer.
        trailers.insert(
            TAG_CHANGE_ID.into(),
            format!("I{}", Self::generate_hash(&repo)),
        );

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
        while lines.len() > 1 {
            if !lines.last().unwrap().is_empty() {
                break;
            }
            lines.pop();
        }

        // ... then, process trailers...
        let mut trailers = BTreeMap::new();
        let trailer_regex = Regex::new(r"^([a-zA-Z0-9-]+):\s*(.*)\s*$").unwrap();

        while lines.len() > 1 {
            let line = lines.last().unwrap();
            if let Some(caps) = trailer_regex.captures(line) {
                info!("Found trailer {}: {}", &caps[1], &caps[2]);
                trailers.insert(caps[1].to_string(), caps[2].to_string());
                lines.pop();
            } else {
                break;
            }
        }

        // ... then, skip empty lines at end.
        while lines.len() > 1 {
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

        res += "\n\n";
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
        self.config = Some(cfg);
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
        "Gerrit ChangeId generator (built-in)"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn run(&self, _files: &[String], args: &Vec<String>) -> Result<()> {
        Self::generate_change_id(&args[0])
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_should_generate_change_id_never() {
        assert_eq!(
            false,
            GerritChangeIdAction::should_generate_change_id(
                "false",
                "Commit Message",
                &BTreeMap::new()
            )
        );
    }

    #[test]
    fn test_should_generate_change_id_always() {
        assert_eq!(
            true,
            GerritChangeIdAction::should_generate_change_id(
                "always",
                "Commit Message",
                &BTreeMap::new()
            )
        );
    }

    #[test]
    fn test_should_generate_change_id_needed() {
        let mut map = BTreeMap::new();
        map.insert("ChangeId".to_owned(), "1234".to_owned());
        assert_eq!(
            true,
            GerritChangeIdAction::should_generate_change_id("true", "Commit Message", &map)
        );
    }

    #[test]
    fn test_should_generate_change_id_not_needed_fixup_1() {
        assert_eq!(
            false,
            GerritChangeIdAction::should_generate_change_id(
                "true",
                "xxx! Commit Message",
                &BTreeMap::new()
            )
        );
    }

    #[test]
    fn test_should_generate_change_id_not_needed_fixup_2() {
        assert_eq!(
            false,
            GerritChangeIdAction::should_generate_change_id("true", "fixup", &BTreeMap::new())
        );
    }

    #[test]
    fn test_should_generate_change_id_not_needed_fixup_3() {
        assert_eq!(
            false,
            GerritChangeIdAction::should_generate_change_id(
                "true",
                "fixup: asdf",
                &BTreeMap::new()
            )
        );
    }

    #[test]
    fn test_should_generate_change_id_not_needed_change_id_exists() {
        let mut map = BTreeMap::new();
        map.insert(TAG_CHANGE_ID.to_owned(), "1234".to_owned());
        assert_eq!(
            false,
            GerritChangeIdAction::should_generate_change_id("true", "Commit Message", &map)
        );
    }

    #[test]
    fn test_parse_commit_msg_single_line() {
        let (message, trailers) =
            GerritChangeIdAction::parse_commit_message("This is a message").unwrap();
        assert_eq!("This is a message", message);
        assert_eq!(0, trailers.len());
    }

    #[test]
    fn test_parse_commit_msg_single_line_with_newlines() {
        let (message, trailers) =
            GerritChangeIdAction::parse_commit_message("This is a message\n\n\n\n").unwrap();
        assert_eq!("This is a message", message);
        assert_eq!(0, trailers.len());
    }

    #[test]
    fn test_parse_commit_msg_single_line_with_multilines() {
        let (message, trailers) = GerritChangeIdAction::parse_commit_message(
            "This is a message\n\nThis is a description\n\n",
        )
        .unwrap();
        assert_eq!("This is a message\n\nThis is a description", message);
        assert_eq!(0, trailers.len());
    }

    #[test]
    fn test_parse_commit_msg_single_line_with_trailer() {
        let (message, trailers) = GerritChangeIdAction::parse_commit_message(
            "This is a message\n\nThis is a description\n\nKey: Value",
        )
        .unwrap();
        assert_eq!("This is a message\n\nThis is a description", message);
        assert_eq!(1, trailers.len());
        assert_eq!("Value", trailers.get("Key").unwrap());
    }

    #[test]
    fn test_parse_commit_msg_single_line_with_trailers() {
        let (message, trailers) = GerritChangeIdAction::parse_commit_message(
            "This is a message\n\nThis is a description\n\nKey: Value\nKey2: Value2",
        )
        .unwrap();
        assert_eq!("This is a message\n\nThis is a description", message);
        assert_eq!(2, trailers.len());
        assert_eq!("Value", trailers.get("Key").unwrap());
        assert_eq!("Value2", trailers.get("Key2").unwrap());
    }

    #[test]
    fn test_parse_commit_msg_single_line_with_bogus_trailer() {
        let (message, trailers) = GerritChangeIdAction::parse_commit_message(
            "This is a message\n\nNot: Trailer\n\nYes: Trailer\n",
        )
        .unwrap();
        assert_eq!("This is a message\n\nNot: Trailer", message);
        assert_eq!(1, trailers.len());
        assert_eq!("Trailer", trailers.get("Yes").unwrap());
    }

    #[test]
    fn test_parse_commit_msg_title_looks_like_trailer() {
        let (message, trailers) =
            GerritChangeIdAction::parse_commit_message("This: Not a trailer\n").unwrap();
        assert_eq!("This: Not a trailer", message);
        assert_eq!(0, trailers.len());
    }

    #[test]
    fn test_parse_commit_msg_bad_format() {
        assert!(!GerritChangeIdAction::parse_commit_message("Line1\nLine2").is_ok());
    }
}
