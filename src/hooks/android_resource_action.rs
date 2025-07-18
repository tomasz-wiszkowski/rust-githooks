use std::fs;

use anyhow::Result;

use log::warn;
use regex::Regex;
use roxmltree::{Document, Node, NodeType};
use serde_derive::Deserialize;
use std::sync::OnceLock;

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

#[derive(Debug, Clone)]
enum CommentType {
    PrefixComment,
    SuffixComment,
}

#[derive(Debug)]
struct SuffixCommentRule {
    name: &'static str,
    pattern: &'static str,
}

const SUFFIX_COMMENT_RULES: &[SuffixCommentRule] = &[SuffixCommentRule {
    name: "lint_then_change",
    pattern: r"LINT\.ThenChange",
}];

static COMPILED_RULES: OnceLock<Vec<(String, Regex)>> = OnceLock::new();

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

const INDENT: &str = "    ";
const WRAP_INDENT: &str = "    ";
const LINE_LIMIT: usize = 100;

/// Classifies a comment based on its content
fn classify_comment(text: &str) -> CommentType {
    let compiled_rules = COMPILED_RULES.get_or_init(|| {
        SUFFIX_COMMENT_RULES
            .iter()
            .map(|rule| (rule.name.to_string(), Regex::new(rule.pattern).unwrap()))
            .collect()
    });

    for (_name, pattern) in compiled_rules {
        if pattern.is_match(text) {
            return CommentType::SuffixComment;
        }
    }
    CommentType::PrefixComment
}

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
        match node.node_type() {
            NodeType::Comment => {
                let text = node.text().unwrap_or("").trim();
                let comment_type = classify_comment(text);

                // Apply spacing rules based on comment type and position
                let should_add_linebreak = match comment_type {
                    CommentType::PrefixComment => add_linebreak, // Respect position-based logic
                    CommentType::SuffixComment => add_linebreak, // Adjacent to previous
                };

                if should_add_linebreak {
                    out.push('\n');
                }

                let pad = INDENT.repeat(indent);
                // TODO: reflow comments.
                if text.contains('\n') {
                    let text = pad.clone() + text;
                    let processed_text = process_comment(&text, pad.len());
                    out.push_str(&format!("{pad}<!--\n{processed_text}\n{pad}-->\n"));
                } else {
                    out.push_str(&format!("{pad}<!-- {text} -->\n"));
                }

                // Return spacing behavior for next element
                match comment_type {
                    CommentType::PrefixComment => false, // Adjacent to next
                    CommentType::SuffixComment => true,  // Empty line after
                }
            }

            NodeType::Element => {
                if add_linebreak {
                    out.push('\n');
                }
                Self::format_element(node, indent, out)
            }

            _ => {
                if add_linebreak {
                    out.push('\n');
                }
                false
            }
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
                out.push_str(&format!("\n{pad}{WRAP_INDENT}{a}"));
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
        for child in children.iter() {
            match child.node_type() {
                NodeType::Comment => {
                    let text = child.text().unwrap_or("").trim();
                    let comment_type = classify_comment(text);

                    let should_add_linebreak = match comment_type {
                        CommentType::PrefixComment => !is_first, // No empty line if first position
                        CommentType::SuffixComment => false,     // Adjacent to previous
                    };

                    add_newline = Self::format_node(*child, indent + 1, should_add_linebreak, out);
                }
                NodeType::Element => {
                    add_newline = Self::format_node(*child, indent + 1, add_newline, out);
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

    #[test]
    fn test_wrap_indent_constants() {
        assert_eq!(INDENT, "    ");
        assert_eq!(WRAP_INDENT, "    ");
        assert_eq!(INDENT.len(), 4);
        assert_eq!(WRAP_INDENT.len(), 4);
    }

    #[test]
    fn test_multiline_attributes_use_wrap_indent() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><TextView xmlns:android="http://schemas.android.com/apk/res/android" android:id="@+id/very_long_id_name" android:layout_width="match_parent" android:layout_height="wrap_content" android:text="Sample" />"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // Should contain wrapped attributes with WRAP_INDENT (4 spaces)
        assert!(output.contains("    android:id="));
        assert!(output.contains("    android:layout_width="));
        assert!(output.contains("    android:layout_height="));

        // Should NOT contain double INDENT (8 spaces) for attributes
        assert!(!output.contains("        android:id="));
    }

    #[test]
    fn test_nested_element_indentation() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><resources><string-array name="test"><item>value</item></string-array></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // Nested string-array should use INDENT (4 spaces)
        assert!(output.contains("    <string-array"));

        // Nested item should use 2*INDENT (8 spaces)
        assert!(output.contains("        <item>"));
    }

    #[test]
    fn test_text_content_indentation() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><resources><string name="test">Long text content that should be indented properly</string></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // Text content should be inline for short content
        assert!(output.contains(
            r#"<string name="test">Long text content that should be indented properly</string>"#
        ));
    }

    #[test]
    fn test_inline_vs_multiline_attribute_threshold() {
        // Test short attributes stay inline
        let short_input =
            r#"<?xml version="1.0" encoding="utf-8"?><item name="short" type="style" />"#;
        let doc = Document::parse(short_input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // Should be inline (no newlines before attributes)
        assert!(output.contains(r#"<item name="short" type="style"/>"#));

        // Test long attributes wrap
        let long_input = r#"<?xml version="1.0" encoding="utf-8"?><TextView xmlns:android="http://schemas.android.com/apk/res/android" android:id="@+id/very_long_identifier_name" android:layout_width="match_parent" android:layout_height="wrap_content" android:text="Very long text content here" />"#;
        let doc_long = Document::parse(long_input).unwrap();
        let output_long = AndroidResourceFormatterAction::format_doc(&doc_long);

        // Should wrap (contains newlines before attributes)
        assert!(output_long.contains("\n    android:id="));
    }

    #[test]
    fn test_comment_indentation() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><resources><!-- Top level comment --><string name="test">value</string></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // Comment should use INDENT (4 spaces)
        assert!(output.contains("    <!-- Top level comment -->"));
    }

    #[test]
    fn test_prefix_comment_spacing() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><resources><string name="first">value</string><!-- Regular comment --><string name="second">value</string></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // Should have empty line before comment and be adjacent to next element
        assert!(output.contains("    <string name=\"first\">value</string>\n\n    <!-- Regular comment -->\n    <string name=\"second\">value</string>"));
    }

    #[test]
    fn test_first_position_comment_spacing() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><resources><!-- First comment --><string name="first">value</string></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // First position comment should not have empty line before
        assert!(output.contains(
            "<resources>\n    <!-- First comment -->\n    <string name=\"first\">value</string>"
        ));
    }

    #[test]
    fn test_suffix_comment_spacing() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><resources><string name="first">value</string><!-- LINT.ThenChange(//path/to/file) --><string name="second">value</string></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // Should be adjacent to previous element and have empty line after
        assert!(output.contains("    <string name=\"first\">value</string>\n    <!-- LINT.ThenChange(//path/to/file) -->\n\n    <string name=\"second\">value</string>"));
    }

    #[test]
    fn test_first_position_suffix_comment_spacing() {
        let input = r#"<?xml version="1.0" encoding="utf-8"?><resources><!-- LINT.ThenChange(//path/to/file) --><string name="first">value</string></resources>"#;
        let doc = Document::parse(input).unwrap();
        let output = AndroidResourceFormatterAction::format_doc(&doc);

        // First position suffix comment should not have empty line before, but should have empty line after
        assert!(output.contains("<resources>\n    <!-- LINT.ThenChange(//path/to/file) -->\n\n    <string name=\"first\">value</string>"));
    }

    #[test]
    fn test_comment_classification() {
        assert!(matches!(
            classify_comment("Regular comment"),
            CommentType::PrefixComment
        ));
        assert!(matches!(
            classify_comment("LINT.ThenChange(//file)"),
            CommentType::SuffixComment
        ));
        assert!(matches!(
            classify_comment("Another regular comment"),
            CommentType::PrefixComment
        ));
    }
}
