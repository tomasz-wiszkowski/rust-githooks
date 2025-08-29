use anyhow::{bail, Result};
use log::warn;
use serde_derive::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::repo::{GitConfig, Item};

use super::action::ActionTraitInternal;
use super::ActionTrait;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChromeNullMarkedAction {
    #[serde(skip_deserializing)]
    enabled: bool,
    #[serde(skip_deserializing)]
    config: Option<Box<dyn GitConfig>>,
}

const KEY_ENABLED: &str = "enabled";
const VALUE_TRUE: &str = "true";

impl ActionTraitInternal for ChromeNullMarkedAction {
    fn check_valid(&self) -> Result<()> {
        Ok(())
    }

    fn set_config(&mut self, cfg: Box<dyn crate::repo::GitConfig>) -> Result<()> {
        self.enabled = cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE;
        self.config = Some(cfg);
        Ok(())
    }
}

impl ActionTrait for ChromeNullMarkedAction {
    fn name(&self) -> &str {
        "Chrome @NullMarked check"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn is_available(&self) -> bool {
        true
    }

    fn is_selected(&self) -> bool {
        self.enabled
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

    fn run(&self, items: &[Item], _args: &Vec<String>) -> Result<()> {
        for item in items {
            if let Item::File(path_str) = item {
                let path = Path::new(path_str);
                if !path.extension().map_or(false, |e| e == "java") {
                    continue;
                }

                if path
                    .file_name()
                    .map_or(false, |n| n.to_string_lossy().ends_with("Test.java"))
                {
                    continue;
                }

                let file = File::open(path)?;
                let reader = BufReader::with_capacity(8192, file);

                let mut has_import = false;
                let mut has_annotation = false;

                for line in reader.lines() {
                    let line = line?;
                    if line.contains("import org.chromium.build.annotations.NullMarked;") {
                        has_import = true;
                    }
                    if line.trim().starts_with("@NullMarked") {
                        has_annotation = true;
                    }
                    if has_import && has_annotation {
                        break;
                    }
                }

                if !has_import || !has_annotation {
                    bail!(
                        "File {} is missing @NullMarked annotation or import.",
                        path.display()
                    );
                }
            }
        }
        Ok(())
    }
}
