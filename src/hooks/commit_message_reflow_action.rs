use anyhow::Result;
use log::{info, warn};
use regex::Regex;
use serde_derive::Deserialize;
use std::fs;
use std::sync::OnceLock;

use crate::repo::GitConfig;
use crate::repo::Item;

use super::action::ActionTraitInternal;
use super::ActionTrait;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessageReflowAction {
    #[serde(skip_deserializing)]
    enabled: bool,
    #[serde(skip_deserializing)]
    config: Option<Box<dyn GitConfig>>,
}

const KEY_ENABLED: &str = "enabled";
const VALUE_TRUE: &str = "true";
const LINE_LIMIT: usize = 70;

static TRAILER_REGEX: OnceLock<Regex> = OnceLock::new();
static INVALID_TRAILER_REGEX: OnceLock<Regex> = OnceLock::new();
static BULLET_REGEX: OnceLock<Regex> = OnceLock::new();

// A well-formed trailer, e.g. `Change-Id: I1234` or `Signed-off-by: Name`.
fn trailer_regex() -> &'static Regex {
    TRAILER_REGEX.get_or_init(|| Regex::new(r"^[A-Za-z0-9-]+:\s*.*$").unwrap())
}

// A trailer-shaped line using `=` instead of `: `, e.g. `Change-Id=I1234`.
fn invalid_trailer_regex() -> &'static Regex {
    INVALID_TRAILER_REGEX.get_or_init(|| Regex::new(r"^([A-Za-z][A-Za-z0-9_-]*)\s*=\s*(.*)$").unwrap())
}

fn bullet_regex() -> &'static Regex {
    BULLET_REGEX.get_or_init(|| Regex::new(r"^\s*([-*+]|\d+[.)])\s").unwrap())
}

// Normalizes a trailer key to the conventional `Word-word-word` casing, e.g.
// `SIGNED-OFF-BY` or `signed_off_by` -> `Signed-off-by`.
fn normalize_trailer_key(key: &str) -> String {
    let lower = key.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => lower,
    }
}

impl CommitMessageReflowAction {
    // Greedily fills lines up to `width` columns, never splitting a word.
    fn wrap_paragraph(text: &str, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();

        for word in text.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
            } else if current.len() + 1 + word.len() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    // Reflows prose paragraphs, but passes bullet-point lines through untouched
    // and never merges two paragraphs separated by a blank line.
    fn reflow_paragraphs(lines: &[&str], width: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut paragraph: Vec<&str> = Vec::new();

        for &line in lines {
            let is_blank = line.trim().is_empty();
            let is_bullet = bullet_regex().is_match(line);

            if is_blank || is_bullet {
                if !paragraph.is_empty() {
                    out.extend(Self::wrap_paragraph(&paragraph.join(" "), width));
                    paragraph.clear();
                }
                out.push(if is_blank {
                    String::new()
                } else {
                    line.to_string()
                });
            } else {
                paragraph.push(line.trim());
            }
        }

        if !paragraph.is_empty() {
            out.extend(Self::wrap_paragraph(&paragraph.join(" "), width));
        }

        out
    }

    fn reflow_commit_message(content: &str) -> String {
        // Comment lines (git's instructional template, status output, etc.)
        // are discarded entirely, wherever they appear.
        let lines: Vec<&str> = content
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect();

        let Some((&first_line, body)) = lines.split_first() else {
            return content.to_string();
        };

        if first_line.chars().count() > LINE_LIMIT {
            warn!(
                "Commit message summary line is {} characters long (limit {}): {}",
                first_line.chars().count(),
                LINE_LIMIT,
                first_line
            );
        }

        let mut body_lines: Vec<&str> = body.to_vec();
        while body_lines.first().is_some_and(|l| l.trim().is_empty()) {
            body_lines.remove(0);
        }
        while body_lines.last().is_some_and(|l| l.trim().is_empty()) {
            body_lines.pop();
        }

        // Trailers are a contiguous block of `Key: value` lines at the very
        // end of the message - pop them off untouched before reflowing,
        // fixing up any that use `Key=value` instead of `Key: value`.
        let mut trailer_lines: Vec<String> = Vec::new();
        while let Some(&line) = body_lines.last() {
            if trailer_regex().is_match(line) {
                trailer_lines.push(line.to_string());
                body_lines.pop();
            } else if let Some(caps) = invalid_trailer_regex().captures(line) {
                let key = normalize_trailer_key(&caps[1]);
                let value = caps[2].trim();
                info!("Fixing malformed trailer `{}` -> `{}: {}`", line, key, value);
                trailer_lines.push(format!("{}: {}", key, value));
                body_lines.pop();
            } else {
                break;
            }
        }
        trailer_lines.reverse();

        while body_lines.last().is_some_and(|l| l.trim().is_empty()) {
            body_lines.pop();
        }

        let reflowed_body = Self::reflow_paragraphs(&body_lines, LINE_LIMIT);

        let mut result = first_line.to_string();
        if !reflowed_body.is_empty() {
            result.push_str("\n\n");
            result.push_str(&reflowed_body.join("\n"));
        }
        if !trailer_lines.is_empty() {
            result.push_str("\n\n");
            result.push_str(&trailer_lines.join("\n"));
        }
        result.push('\n');

        result
    }

    fn reflow_file(file_path: &str) -> Result<()> {
        let content = fs::read_to_string(file_path)?;
        let reflowed = Self::reflow_commit_message(&content);
        Ok(fs::write(file_path, reflowed)?)
    }
}

impl ActionTraitInternal for CommitMessageReflowAction {
    fn check_valid(&self) -> Result<()> {
        Ok(())
    }

    fn set_config(&mut self, cfg: Box<dyn crate::repo::GitConfig>) -> Result<()> {
        self.enabled = cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE;
        self.config = Some(cfg);
        Ok(())
    }
}

impl ActionTrait for CommitMessageReflowAction {
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
        "Commit message reflow (built-in)"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn run(&self, _items: &[Item], args: &Vec<String>) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let Some(file_path) = args.first() else {
            anyhow::bail!("Commit message reflow requires a path to the commit message file.");
        };

        Self::reflow_file(file_path)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_wrap_paragraph_short() {
        let lines = CommitMessageReflowAction::wrap_paragraph("Short text", 80);
        assert_eq!(lines, vec!["Short text".to_string()]);
    }

    #[test]
    fn test_wrap_paragraph_wraps_at_width() {
        let text = "one two three four five six seven eight nine ten eleven twelve thirteen";
        let lines = CommitMessageReflowAction::wrap_paragraph(text, 20);
        assert!(lines.iter().all(|l| l.len() <= 20));
        assert_eq!(lines.join(" "), text);
    }

    #[test]
    fn test_wrap_paragraph_overlong_word_not_split() {
        let text = "a-very-long-word-that-does-not-fit-within-the-limit-by-itself-at-all";
        let lines = CommitMessageReflowAction::wrap_paragraph(text, 10);
        assert_eq!(lines, vec![text.to_string()]);
    }

    #[test]
    fn test_reflow_paragraphs_preserves_bullets() {
        let lines = vec![
            "This is a long sentence that should be reflowed because it has",
            "no bullet marker at the start of the line at all really.",
            "",
            "- bullet one stays exactly as written even if quite long indeed",
            "- bullet two",
            "* star bullets are recognised too",
            "1. numbered bullets as well",
        ];
        let out = CommitMessageReflowAction::reflow_paragraphs(&lines, 80);

        assert!(out.contains(&"- bullet one stays exactly as written even if quite long indeed".to_string()));
        assert!(out.contains(&"- bullet two".to_string()));
        assert!(out.contains(&"* star bullets are recognised too".to_string()));
        assert!(out.contains(&"1. numbered bullets as well".to_string()));
        assert!(out.contains(&String::new()));
    }

    #[test]
    fn test_reflow_paragraphs_does_not_merge_across_blank_line() {
        let lines = vec!["Paragraph one.", "", "Paragraph two."];
        let out = CommitMessageReflowAction::reflow_paragraphs(&lines, 80);
        assert_eq!(out, vec!["Paragraph one.", "", "Paragraph two."]);
    }

    #[test]
    fn test_reflow_commit_message_keeps_long_first_line() {
        let first_line = "x".repeat(120);
        let content = format!("{}\n", first_line);
        let result = CommitMessageReflowAction::reflow_commit_message(&content);
        assert_eq!(result, format!("{}\n", first_line));
    }

    #[test]
    fn test_reflow_commit_message_wraps_body() {
        let body = "word ".repeat(40);
        let content = format!("Subject\n\n{}\n", body.trim());
        let result = CommitMessageReflowAction::reflow_commit_message(&content);

        for line in result.lines() {
            assert!(line.chars().count() <= LINE_LIMIT, "line too long: {}", line);
        }
    }

    #[test]
    fn test_reflow_commit_message_does_not_merge_paragraphs() {
        let content = "Subject\n\nFirst paragraph stays separate.\n\nSecond paragraph stays separate too.\n";
        let result = CommitMessageReflowAction::reflow_commit_message(content);
        assert_eq!(
            result,
            "Subject\n\nFirst paragraph stays separate.\n\nSecond paragraph stays separate too.\n"
        );
    }

    #[test]
    fn test_reflow_commit_message_preserves_trailers_verbatim() {
        let long_trailer_value = "y".repeat(120);
        let content = format!(
            "Subject\n\nBody text here.\n\nChange-Id: I123\nLink: {}\n",
            long_trailer_value
        );
        let result = CommitMessageReflowAction::reflow_commit_message(&content);

        assert!(result.contains("Change-Id: I123"));
        assert!(result.contains(&format!("Link: {}", long_trailer_value)));
    }

    #[test]
    fn test_reflow_commit_message_fixes_invalid_trailer_syntax() {
        let content = "Subject\n\nBody text here.\n\nCHANGE-ID=I123\nReviewed-by: Jane Doe\n";
        let result = CommitMessageReflowAction::reflow_commit_message(content);

        assert!(result.contains("Change-id: I123"));
        assert!(result.contains("Reviewed-by: Jane Doe"));
        assert!(!result.contains("CHANGE-ID=I123"));
    }

    #[test]
    fn test_reflow_commit_message_fixes_invalid_trailer_keeps_value_unwrapped() {
        let long_value = "z".repeat(120);
        let content = format!("Subject\n\nBody text here.\n\nLink={}\n", long_value);
        let result = CommitMessageReflowAction::reflow_commit_message(&content);

        assert!(result.contains(&format!("Link: {}", long_value)));
    }

    #[test]
    fn test_reflow_commit_message_preserves_bullets_untouched() {
        let content = "Subject\n\n- first bullet point\n- second bullet point\n";
        let result = CommitMessageReflowAction::reflow_commit_message(content);
        assert_eq!(
            result,
            "Subject\n\n- first bullet point\n- second bullet point\n"
        );
    }

    #[test]
    fn test_reflow_commit_message_strips_comment_lines_anywhere() {
        let content = "Subject\n\n# stray comment mid-message\nBody.\n\n# Please enter the commit message.\n# On branch main\n";
        let result = CommitMessageReflowAction::reflow_commit_message(content);
        assert_eq!(result, "Subject\n\nBody.\n");
    }

    #[test]
    fn test_reflow_commit_message_no_body_no_trailers() {
        let content = "Subject only\n";
        let result = CommitMessageReflowAction::reflow_commit_message(content);
        assert_eq!(result, "Subject only\n");
    }
}
