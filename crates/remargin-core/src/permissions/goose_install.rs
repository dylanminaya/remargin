//! Lifecycle of the goose hook plugin that dispatches to `remargin goose
//! pretool`.
//!
//! Operates over a plugin *directory* (caller picks user-scope or
//! project-scope) rather than a settings file: goose discovers plugins by
//! their presence on disk, with no registration step. Idempotent.
//!
//! Two properties of the generated `hooks.json` are load-bearing:
//!
//! - **No `matcher` key.** goose reads `matcher` as a regex; a bare `*` is
//!   invalid and the whole entry is dropped with only a log warning — a
//!   guard silently absent. Omitting the key matches every event of the
//!   type, which is what the guard wants.
//! - **An absolute binary path.** goose fails open when the hook command
//!   cannot be spawned, so a `PATH` miss at spawn time is an unguarded
//!   session with no signal. The path is resolved from the running
//!   executable at install time.

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use os_shim::System;
use serde_json::{Value, json};

/// Plugin directory name under `<root>/.agents/plugins/`.
pub const PLUGIN_NAME: &str = "remargin-guard";

/// The only goose hook event that can block a tool call.
pub const HOOK_EVENT: &str = "PreToolUse";

/// Subcommand appended to the absolute binary path in the generated hook
/// command.
pub const HOOK_SUBCOMMAND: &str = "goose pretool";

/// Seconds goose waits for the guard before abandoning it. Generous
/// because abandoning it is a fail-open pass for the tool call.
const HOOK_TIMEOUT_SECS: u32 = 30;

/// Plugin manifest, relative to the plugin directory.
const PLUGIN_MANIFEST: &str = "plugin.json";

/// Hook manifest, relative to the plugin directory.
const HOOKS_MANIFEST: &str = "hooks/hooks.json";

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum InstallOutcome {
    AlreadyInstalled,
    Installed,
}

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TestOutcome {
    /// The plugin directory is present but does not describe a live guard.
    /// Carries the specific fault so the caller can name it.
    Broken(String),
    Installed,
    NotInstalled,
}

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UninstallOutcome {
    NotInstalled,
    Uninstalled,
}

/// The directory whose presence marks `root` as a goose plugin scope.
#[must_use]
pub fn agents_dir(root: &Path) -> PathBuf {
    root.join(".agents")
}

/// The guard's plugin directory under `root` — a home directory for
/// user scope, a project directory for `--local`.
#[must_use]
pub fn plugin_dir(root: &Path) -> PathBuf {
    agents_dir(root).join("plugins").join(PLUGIN_NAME)
}

/// Write the plugin directory, rewriting drifted content in place.
///
/// # Errors
///
/// Returns an error when the running executable cannot be resolved (the
/// hook command would otherwise be written without an absolute path) or
/// when the plugin files cannot be written.
pub fn install(system: &dyn System, dir: &Path) -> Result<InstallOutcome> {
    let command = hook_command(system)?;
    let plugin_body = render(&plugin_manifest())?;
    let hooks_body = render(&hooks_manifest(&command))?;
    let plugin_file = dir.join(PLUGIN_MANIFEST);
    let hooks_file = dir.join(HOOKS_MANIFEST);

    if file_matches(system, &plugin_file, &plugin_body)
        && file_matches(system, &hooks_file, &hooks_body)
    {
        return Ok(InstallOutcome::AlreadyInstalled);
    }
    write_file(system, &plugin_file, &plugin_body)?;
    write_file(system, &hooks_file, &hooks_body)?;
    Ok(InstallOutcome::Installed)
}

/// Remove the guard's plugin directory, leaving sibling plugins alone.
///
/// # Errors
///
/// Returns an error when the directory exists but cannot be removed.
pub fn uninstall(system: &dyn System, dir: &Path) -> Result<UninstallOutcome> {
    if !system.exists(dir)? {
        return Ok(UninstallOutcome::NotInstalled);
    }
    system
        .remove_dir_all(dir)
        .with_context(|| format!("removing goose plugin directory {}", dir.display()))?;
    Ok(UninstallOutcome::Uninstalled)
}

/// Report whether the guard is wired: the directory is present, the hook
/// manifest parses, it declares a `PreToolUse` command entry, and that
/// command's binary exists on disk.
///
/// # Errors
///
/// Returns an error when a path existence probe fails.
pub fn test(system: &dyn System, dir: &Path) -> Result<TestOutcome> {
    if !system.exists(dir)? {
        return Ok(TestOutcome::NotInstalled);
    }
    let hooks_file = dir.join(HOOKS_MANIFEST);
    let Ok(body) = system.read_to_string(&hooks_file) else {
        return Ok(broken(&format!("{} is unreadable", hooks_file.display())));
    };
    let value: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(err) => {
            return Ok(broken(&format!(
                "{} is not valid JSON ({err})",
                hooks_file.display()
            )));
        }
    };
    let Some(command) = declared_hook_command(&value) else {
        return Ok(broken(&format!(
            "{} declares no {HOOK_EVENT} command entry",
            hooks_file.display()
        )));
    };
    let binary = command_binary(command);
    if !system.exists(Path::new(binary))? {
        return Ok(broken(&format!(
            "the hook command in {} points at {binary}, which does not exist",
            hooks_file.display()
        )));
    }
    Ok(TestOutcome::Installed)
}

fn broken(reason: &str) -> TestOutcome {
    TestOutcome::Broken(String::from(reason))
}

/// The hook command: the absolute path to the running remargin binary plus
/// the dispatch subcommand. Written unquoted so it reads the same whether
/// goose spawns it through a shell or splits it into argv.
fn hook_command(system: &dyn System) -> Result<String> {
    let exe = system
        .current_exe()
        .context("resolving the remargin binary path for the goose hook command")?;
    Ok(format!("{} {HOOK_SUBCOMMAND}", exe.display()))
}

/// The binary path out of a generated hook command — everything ahead of
/// the subcommand the installer appends. Splitting on the suffix rather
/// than on whitespace keeps a binary path containing spaces intact.
fn command_binary(command: &str) -> &str {
    command
        .strip_suffix(HOOK_SUBCOMMAND)
        .map_or(command, str::trim_end)
}

fn declared_hook_command(value: &Value) -> Option<&str> {
    value
        .get("hooks")?
        .get(HOOK_EVENT)?
        .as_array()?
        .iter()
        .find_map(|entry| {
            entry.get("hooks")?.as_array()?.iter().find_map(|hook| {
                let is_command = hook.get("type").and_then(Value::as_str) == Some("command");
                is_command
                    .then(|| hook.get("command").and_then(Value::as_str))
                    .flatten()
            })
        })
}

fn plugin_manifest() -> Value {
    json!({
        "name": PLUGIN_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "description":
            "Blocks goose file and shell tools on remargin-managed paths and redirects the \
             agent to the remargin MCP tools.",
    })
}

fn hooks_manifest(command: &str) -> Value {
    json!({
        "hooks": {
            HOOK_EVENT: [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": command,
                            "timeout": HOOK_TIMEOUT_SECS,
                        },
                    ],
                },
            ],
        },
    })
}

fn render(value: &Value) -> Result<String> {
    let mut body = serde_json::to_string_pretty(value).context("serializing goose plugin JSON")?;
    body.push('\n');
    Ok(body)
}

fn file_matches(system: &dyn System, path: &Path, body: &str) -> bool {
    system.read_to_string(path).is_ok_and(|found| found == body)
}

fn write_file(system: &dyn System, path: &Path, body: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        system
            .create_dir_all(parent)
            .with_context(|| format!("creating goose plugin directory {}", parent.display()))?;
    }
    system
        .write(path, body.as_bytes())
        .with_context(|| format!("writing goose plugin file {}", path.display()))
}
