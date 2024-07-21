use anyhow::{Context, Result};
use log::warn;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use std::collections::HashMap;

/**
 * Resolve executable name into full path.
 * Searches for executableName either in PATH or location relative to
 * current path. When found, returns Some with absolute path of the command.
 * Otherwise returns None.
 */
pub fn get_shell_command_absolute_path(executable_name: &str) -> Option<PathBuf> {
    if let Ok(path) = which::which(executable_name) {
        return Some(path);
    }

    let work_dir = env::current_dir()
        .context("Resolve: cannot determine current dir")
        .ok()?;
    let abs_path = work_dir.join(executable_name);

    if abs_path.exists() {
        Some(abs_path)
    } else {
        None
    }
}
/**
 * Execute supplied shell command.
 * The command must be supplied in an "exploded" form, where each argument is a
 * separate string. Returns a tuple of strings: (stdout, stderr).
 */
pub fn run_shell_command(args: &[String]) -> Result<(String, String)> {
    let output = Command::new(&args[0])
        .args(&args[1..])
        .output()
        .context("Failed to execute command")?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        warn!("Command failed");
        let stdout = stdout.trim();
        if !stdout.is_empty() {
            warn!("stdout:\n{}", stdout);
        }
        let stderr = stderr.trim();
        if !stderr.is_empty() {
            warn!("stderr:\n{}", stderr);
        }
    }

    Ok((stdout, stderr))
}

pub enum Substitution<'a> {
    Scalar(String),
    Array(&'a Vec<String>),
}

/**
 * Substitute arguments and construct a command line.
 * The input_cmd_line represents the set of arguments (including command).
 * Each argument is matched against items in substitute_args map. When a
 * corresponding entry is found, the argument is replaced with the
 * map value. Replaces single strings and string arrays.
 * Returns resulting CommandLine.
 */
pub fn substitute_command_line(
    input_cmd_line: &[String],
    substitute_args: &HashMap<String, Substitution>,
) -> Vec<String> {
    let mut out = Vec::new();

    for arg in input_cmd_line {
        if let Some(sub) = substitute_args.get(arg) {
            match sub {
                Substitution::Scalar(s) => out.push(s.clone()),
                Substitution::Array(a) => out.extend(a.iter().map(|s| s.clone())),
            }
        } else {
            out.push(arg.clone());
        }
    }
    out
}
