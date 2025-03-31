use std::fs;

use anyhow::Result;

use log::warn;
use roxmltree::{Document, Node, NodeType};
use serde_derive::Deserialize;

use crate::repo::GitConfig;
use crate::repo::Item;

use super::action::ActionTraitInternal;
use super::ActionTrait;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AndroidResourceFormatterAction {
    #[serde(skip_deserializing)]
    enabled: bool,
    #[serde(skip_deserializing)]
    config: Option<Box<dyn GitConfig>>,
}

const KEY_ENABLED: &str = "enabled";
const VALUE_TRUE: &str = "true";

impl ActionTraitInternal for AndroidResourceFormatterAction {
    fn check_valid(&self) -> Result<()> {
        Ok(())
    }

    fn set_config(&mut self, cfg: Box<dyn crate::repo::GitConfig>) -> Result<()> {
        self.enabled = cfg.get_or_default(KEY_ENABLED, "") == VALUE_TRUE;
        self.config = Some(cfg);
        Ok(())
    }
}

impl AndroidResourceFormatterAction {
    fn format_file(&self, infile: &str) -> Result<()> {
        let input = fs::read_to_string(infile).expect("Failed to read input.xml");
        let doc = Document::parse(&input).expect("Failed to parse XML");

        let mut output = String::new();
        for node in doc.root().children() {
            self.format_node(&mut output, &node, 0);
        }

        let outfile = infile.to_owned() + ".fmt";
        Ok(fs::write(outfile, output.as_bytes())?)
    }

    fn format_node(&self, output: &mut String, node: &Node, indent_level: usize) {
        match node.node_type() {
            NodeType::Element => self.format_element(output, node, indent_level),
            NodeType::Text => {
                let text = node.text().unwrap_or("");
                if text.trim().is_empty() {
                    output.push('\n');
                } else {
                    output.push_str(text.trim());
                }
            }
            NodeType::Comment => {
                let indent = "  ".repeat(indent_level);
                output.push_str(&format!("{}<!--{}-->\n", indent, node.text().unwrap_or("")));
            }
            NodeType::PI => {
                let indent = "  ".repeat(indent_level);
                output.push_str(&format!("{}<?{}?>\n", indent, node.text().unwrap_or("")));
            }
            _ => {}
        }
    }

    fn format_element(&self, output: &mut String, node: &Node, indent_level: usize) {
        let indent = "  ".repeat(indent_level);
        output.push_str(&format!("{}<{}", indent, node.tag_name().name()));

        for attr in node.attributes() {
            output.push_str(&format!(" {}=\"{}\"", attr.name(), attr.value()));
        }

        if !node.has_children() {
            output.push_str("/>");
        } else {
            output.push_str(">");

            for child in node.children() {
                match child.node_type() {
                    NodeType::Element | NodeType::Comment | NodeType::PI | NodeType::Text => {
                        self.format_node(output, &child, indent_level + 1);
                    }
                    _ => {}
                }
            }

            output.push_str(&format!("</{}>", node.tag_name().name()));
        }
    }
}

impl ActionTrait for AndroidResourceFormatterAction {
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
        "Android resource files (xml) formatter"
    }

    fn priority(&self) -> i32 {
        100
    }

    fn run(&self, items: &[Item], _args: &Vec<String>) -> Result<()> {
        items
            .into_iter()
            .filter_map(|i| {
                let Item::File(name) = i else {
                    return None;
                };
                if !name.contains("/java/res/") || !name.ends_with(".xml") {
                    return None;
                }

                return Some(name);
            })
            .map(|i| {
                warn!("Formatting resource file: {}", i);
                self.format_file(i)
            })
            .collect::<Result<_>>()
    }
}
