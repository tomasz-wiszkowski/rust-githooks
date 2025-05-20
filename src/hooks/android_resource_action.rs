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

/// Computes the minimum indentation of the input string.
///
/// This function ignores the first line of the input string when calculating the minimum indentation. This is because the first
/// line may have a different indentation level than the rest of the lines, when it directly follows the XML comment markup.
/// The function returns the minimum indentation level found in the remaining lines, or 0 if there are no lines to process.
///
/// # Arguments
///   input: A string slice that contains the input text.
///
/// # Returns
///   The minimum indentation level found in the input string, or 0 if there are no lines to process.
fn compute_indent(input: &str) -> usize {
    let lines = input.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return 0;
    }
    let min_indent = lines
        .iter()
        .skip(1) // First line may be indented differently, so ignore that
        .filter_map(|line| line.chars().position(|c| !c.is_whitespace()))
        .min()
        .unwrap_or(0);
    min_indent
}

/// Dedents the input string by removing the specified number of leading spaces from each line.
///
/// This function is useful for normalizing the indentation of a block of text. The first line is treated specially, as it may
/// have a different indentation level than the rest of the lines when it directly follows the XML comment markup.
///
/// # Arguments
///  input: A string slice that contains the input text.
///  indent: The number of leading spaces to remove from each line.
///
/// # Returns
///   A new string with the specified number of leading spaces removed from each line.
fn dedent(input: &str, indent: usize) -> String {
    let lines = input.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return String::new();
    }
    lines[0].trim().to_owned()
        + "\n"
        + &lines
            .iter()
            .skip(1)
            .map(|line| &line[indent..])
            .collect::<Vec<_>>()
            .join("\n")
}

/// Indents the input string by adding the specified number of leading spaces to each line.
///
/// This function is useful for formatting a block of text to align with a specific indentation level.
///
/// # Arguments
///   input: A string slice that contains the input text.
///   indent: The number of leading spaces to add to each line.
///
/// # Returns
///   A new string with the specified number of leading spaces added to each line.
fn indent(input: &str, indent: usize) -> String {
    let lines = input.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return String::new();
    }
    lines
        .iter()
        .map(|line| format!("{}{}", " ".repeat(indent), line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Processes the input string by removing the leading spaces and re-indenting it.
///
/// This function is useful for normalizing the indentation of a block of text.
/// It computes the minimum indentation of the input string, removes that indentation from each line,
/// and then re-indents the text to the specified target indentation level.
///
/// # Arguments
///   input: A string slice that contains the input text.
///   target_indent: The target indentation level to apply to the processed text.
///
/// # Returns
///   A new string with the specified target indentation level applied to each line.
fn process_comment(input: &str, target_indent: usize) -> String {
    let def_indent = compute_indent(input);

    let result = dedent(input, def_indent);

    indent(&result, target_indent)
}

const INDENT: &str = "  ";
const LINE_LIMIT: usize = 100;

impl AndroidResourceFormatterAction {
    fn format_file(&self, infile: &str) -> Result<()> {
        let input = fs::read_to_string(infile).expect("Failed to read input.xml");
        let doc = Document::parse(&input).expect("Failed to parse XML");

        let output = Self::format_doc(&doc);

        let outfile = infile.to_owned();
        Ok(fs::write(outfile, output.as_bytes())?)
    }

    fn format_doc(doc: &Document) -> String {
        // roxml strips the initial <?xml ... ?> declaration
        let mut output = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n".to_owned();
        for node in doc.root().children() {
            Self::format_node(node, 0, false, &mut output);
        }
        output
    }

    // ---------------- core recursive formatter ----------------
    fn format_node(node: Node, indent: usize, add_linebreak: bool, out: &mut String) -> bool {
        if add_linebreak {
            out.push('\n');
        }

        match node.node_type() {
            NodeType::Comment => {
                let pad = INDENT.repeat(indent);
                let text = node.text().unwrap_or("").trim();
                // TODO: reflow comments.
                if text.contains('\n') {
                    let text = pad.clone() + text;
                    let processed_text = process_comment(&text, pad.len());
                    out.push_str(&format!("{pad}<!--\n{processed_text}\n{pad}-->\n"));
                } else {
                    out.push_str(&format!("{pad}<!-- {text} -->\n"));
                }
                false
            }

            NodeType::Element => Self::format_element(node, indent, out),

            _ => false,
        }
    }

    fn format_element(node: Node, indent: usize, out: &mut String) -> bool {
        let pad = INDENT.repeat(indent);
        let tag = Self::qualified_tag_name(node);

        // ---------- attributes ----------
        let mut attrs = Vec::new();
        let mut inline_len = tag.len() + 2; // "<tag" + space

        if indent == 0 {
            for a in node.namespaces() {
                let name = a.name().unwrap_or("");
                let uri = a.uri();
                let pair = format!("xmlns:{name}=\"{uri}\"");
                inline_len += 1 + pair.len();
                attrs.push(pair);
            }
        }

        for a in node.attributes() {
            let name = Self::qualified_attr_name(node, &a);
            let escaped_val = Self::escape_attr(a.value());
            let pair = format!("{name}=\"{escaped_val}\"");
            inline_len += 1 + pair.len();
            attrs.push(pair);
        }
        let multiline_attrs = attrs.len() > 3 || inline_len > LINE_LIMIT;

        // ---------- opening tag ----------
        out.push_str(&format!("{pad}<{tag}"));
        for a in &attrs {
            if multiline_attrs {
                out.push_str(&format!("\n{pad}{INDENT}{INDENT}{a}"));
            } else {
                out.push_str(&format!(" {a}"));
            }
        }
        out.push('>');

        // ---------- child analysis ----------
        let children: Vec<_> = node.children().collect();
        let has_elements_or_comments = children
            .iter()
            .any(|c| matches!(c.node_type(), NodeType::Element | NodeType::Comment));
        let text_content = node.text().unwrap_or("").trim();

        let inline_leaf = !has_elements_or_comments
            && !text_content.is_empty()
            && !text_content.contains('\n')
            && (pad.len() + text_content.len() + inline_len + 3) < LINE_LIMIT;

        if inline_leaf {
            out.push_str(&Self::escape_text(text_content));
            out.push_str(&format!("</{tag}>\n"));
            return multiline_attrs;
        }

        if !has_elements_or_comments && text_content.is_empty() {
            // self‑closing tag
            out.pop();
            out.push_str("/>\n");
            return multiline_attrs;
        }

        out.push('\n'); // newline after opening tag

        // print block text if present (escaped)
        if !text_content.is_empty() {
            out.push_str(&format!(
                "{pad}{INDENT}{}\n",
                Self::escape_text(text_content)
            ));
        }

        // recurse into children (elements, comments, additional text nodes)
        let mut add_newline = multiline_attrs;
        let mut is_first = true;
        for child in children {
            match child.node_type() {
                NodeType::Comment => {
                    Self::format_node(child, indent + 1, !is_first, out);
                    add_newline = false;
                }
                NodeType::Element => {
                    add_newline = Self::format_node(child, indent + 1, add_newline, out);
                    is_first = false;
                }
                _ => {}
            }
        }

        if multiline_attrs {
            // If we started with an empty line, end with one too
            out.push('\n');
        }
        out.push_str(&format!("{pad}</{tag}>\n"));

        multiline_attrs || has_elements_or_comments
    }

    // ---------------- helpers ----------------
    fn escape_text(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn escape_attr(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    fn qualified_tag_name(node: Node) -> String {
        match node.tag_name().namespace() {
            Some(ns) => {
                if let Some(prefix) = Self::find_prefix(node, ns) {
                    format!("{prefix}:{}", node.tag_name().name())
                } else {
                    node.tag_name().name().to_string()
                }
            }
            None => node.tag_name().name().to_string(),
        }
    }

    fn qualified_attr_name(node: Node, attr: &roxmltree::Attribute) -> String {
        if let Some(ns) = attr.namespace() {
            if let Some(prefix) = Self::find_prefix(node, ns) {
                return format!("{prefix}:{}", attr.name());
            }
        }
        attr.name().to_string()
    }

    fn find_prefix(mut node: Node, uri: &str) -> Option<String> {
        loop {
            for ns in node.namespaces() {
                if ns.uri() == uri {
                    return ns.name().map(|p| p.to_string());
                }
            }
            node = node.parent()?;
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
            .iter()
            .filter_map(|item| match item {
                Item::File(name) if name.contains("/java/res/") && name.ends_with(".xml") => {
                    Some(name)
                }
                _ => None,
            })
            .try_for_each(|name_of_file_to_format| {
                warn!("Formatting resource file: {}", name_of_file_to_format);
                self.format_file(name_of_file_to_format)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_node() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><!-- Top-level comment --><resources xmlns:android="http://schemas.android.com/apk/res/android" xmlns:app="http://schemas.android.com/apk/res-auto"><!-- A string with escaped characters --><string name="welcome_message">Welcome to &lt;b&gt;MyApp&lt;/b&gt;!</string><!-- A string-array with nested items --><string-array name="days_of_week"><item>Sunday</item><item>Monday</item><item>Tuesday</item><item>Wednesday</item><item>Thursday</item><item>Friday</item><item>Saturday</item></string-array><!-- A plurals element with quantity attributes --><plurals name="number_of_items"><item quantity="zero">No items</item><item quantity="one">One item</item><item quantity="other">%d items</item></plurals><!-- A style with deeply nested items and long attributes --><style name="AppTheme" parent="Theme.MaterialComponents.DayNight.DarkActionBar"><item name="colorPrimary">@color/primary</item><item name="colorPrimaryVariant">@color/primary_variant</item><item name="android:windowBackground">@drawable/bg_main</item></style><!-- A color entry with a comment inline --><color name="primary">#6200EE</color><!-- Primary color --><!-- Dimensions with decimals and comments above --><!-- Spacing used in lists --><dimen name="list_item_spacing">8dp</dimen><!-- Nested elements with attributes that might wrap --><selector><item android:state_pressed="true" android:drawable="@color/primary_dark" /><item android:drawable="@color/primary" /></selector><!-- Include tag example --><include layout="@layout/header" /><!-- Empty element with many attributes --><item name="buttonStyle" type="style" parent="Widget.AppCompat.Button" /><!-- CDATA section example --><string name="html_content"><![CDATA[<html> <body> <h1>Welcome!</h1> </body> </html>]]></string><!-- Multi-attribute element example 1 --><item name="custom_button_style" type="style" parent="Widget.MaterialComponents.Button" /><!-- Multi-attribute element example 2 --><TextView android:id="@+id/sample_text" android:layout_width="match_parent" android:layout_height="wrap_content" android:layout_margin="16dp" android:text="Sample Text" android:textColor="@color/primary" android:textStyle="bold" /><!-- Inline nested element case --><string-array name="single_line_array"><item>OnlyOne</item></string-array><!-- Long attributes with short child inlined --><selector android:layout_width="match_parent" android:layout_height="wrap_content" android:state_enabled="true"><item android:drawable="@color/primary" /></selector><!-- Mixed length children --><string-array name="mixed_length_array"><item>Short</item><item>This is a very long item that might typically be broken into its own line to ensure readability and clarity within the XML formatting context.</item></string-array><!-- Nested elements with comments inside --><plurals name="commented_plural"><!-- Zero case --><item quantity="zero">None</item><!-- Other case with formatting --><item quantity="other">%d items available</item></plurals></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);
        println!("{output}");
    }
}
