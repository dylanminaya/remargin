//! Unit tests for `permissions::goose_mcp_install` — QA scenarios 1-4
//! (install into an empty config, install beside unrelated extensions,
//! uninstall, and the `test` verdicts).
//!
//! The fixtures are the entry shape read off a live goose 1.45.0 config,
//! and every "broken" case is a failure mode observed on that goose: a
//! `cmd` that does not exist, an entry with no `cmd` at all, and an entry
//! turned off with `enabled: false` all leave the session with zero
//! remargin tools while goose starts happily.

use std::path::{Path, PathBuf};

use os_shim::System;
use os_shim::mock::MockSystem;
use serde_yaml::{Mapping, Value};

use super::{
    EXTENSION_KEY, EXTENSION_NAME, InstallOutcome, TestOutcome, UninstallOutcome, install,
    local_config_file, test, uninstall, user_config_file,
};

const EXE: &str = "/opt/bin/remargin";

fn home() -> PathBuf {
    PathBuf::from("/home/u")
}

fn config_path() -> PathBuf {
    PathBuf::from("/home/u/.config/goose/config.yaml")
}

/// A mock whose `current_exe` is the binary the installer must embed.
fn mock() -> MockSystem {
    MockSystem::new()
        .with_current_exe(Path::new(EXE))
        .unwrap()
        .with_file(Path::new(EXE), b"binary")
        .unwrap()
}

fn seed(system: MockSystem, body: &str) -> MockSystem {
    system.with_file(config_path(), body.as_bytes()).unwrap()
}

fn config_of(system: &dyn System) -> Mapping {
    serde_yaml::from_str::<Value>(&system.read_to_string(&config_path()).unwrap())
        .unwrap()
        .as_mapping()
        .unwrap()
        .clone()
}

fn entry_of(system: &dyn System) -> Mapping {
    config_of(system)
        .get(Value::from("extensions"))
        .unwrap()
        .as_mapping()
        .unwrap()
        .get(Value::from(EXTENSION_KEY))
        .unwrap()
        .as_mapping()
        .unwrap()
        .clone()
}

fn field(entry: &Mapping, key: &str) -> Value {
    entry.get(Value::from(key)).cloned().unwrap()
}

fn expect_broken(outcome: TestOutcome) -> String {
    assert!(
        matches!(outcome, TestOutcome::Broken(_)),
        "expected Broken, got {outcome:?}",
    );
    let TestOutcome::Broken(reason) = outcome else {
        return String::new();
    };
    reason
}

/// A hand-written entry in the shape goose accepts, parameterized so each
/// broken-shape test can bend exactly one field.
fn entry_yaml(fields: &str) -> String {
    format!("extensions:\n  remargin:\n{fields}")
}

fn wired_yaml() -> String {
    entry_yaml(
        "    enabled: true\n    type: stdio\n    name: remargin\n    description: managed \
         markdown\n    cmd: /opt/bin/remargin\n    args:\n    - mcp\n    timeout: 300\n",
    )
}

#[test]
fn user_config_file_falls_back_to_dot_config() {
    assert_eq!(user_config_file(&mock(), &home()), config_path());
}

/// `XDG_CONFIG_HOME` relocates goose's config, so the installer follows it
/// rather than assuming `~/.config`.
#[test]
fn user_config_file_follows_xdg_config_home() {
    let system = mock().with_env("XDG_CONFIG_HOME", "/xdg").unwrap();
    assert_eq!(
        user_config_file(&system, &home()),
        PathBuf::from("/xdg/goose/config.yaml"),
    );
}

/// An empty `XDG_CONFIG_HOME` is not a config home; treating it as one
/// would point the installer at `/goose/config.yaml`.
#[test]
fn user_config_file_ignores_an_empty_xdg_config_home() {
    let system = mock().with_env("XDG_CONFIG_HOME", "").unwrap();
    assert_eq!(user_config_file(&system, &home()), config_path());
}

#[test]
fn local_config_file_lands_under_the_project_goose_dir() {
    assert_eq!(
        local_config_file(Path::new("/w/repo")),
        PathBuf::from("/w/repo/.goose/config.yaml"),
    );
}

// ---- 1. install into an empty config -----------------------------------

#[test]
fn install_writes_the_entry_when_no_config_exists() {
    let system = mock();
    assert_eq!(
        install(&system, &config_path()).unwrap(),
        InstallOutcome::Installed,
    );

    let entry = entry_of(&system);
    assert_eq!(field(&entry, "type"), Value::from("stdio"));
    assert_eq!(field(&entry, "enabled"), Value::Bool(true));
    assert_eq!(
        field(&entry, "args"),
        Value::Sequence(vec![Value::from("mcp")]),
    );
    assert_eq!(field(&entry, "timeout"), Value::from(300_u64));
}

/// goose builds tool names from `name`, not from the entry's key, so this
/// field is what makes the guard's `remargin__` allow-prefix match.
#[test]
fn generated_entry_pins_the_name_the_tool_prefix_comes_from() {
    let system = mock();
    let _outcome = install(&system, &config_path()).unwrap();
    assert_eq!(
        entry_of(&system).get(Value::from("name")),
        Some(&Value::from(EXTENSION_NAME)),
    );
}

/// goose warns and continues when it cannot spawn an extension, so a
/// `PATH` miss would be a session that hits the guard with none of the
/// tools its deny message names.
#[test]
fn generated_entry_names_the_binary_by_absolute_path() {
    let system = mock();
    let _outcome = install(&system, &config_path()).unwrap();
    let entry = entry_of(&system);
    let command = field(&entry, "cmd");
    assert_eq!(command, Value::from(EXE));
    assert!(Path::new(command.as_str().unwrap()).is_absolute());
}

#[test]
fn install_is_idempotent() {
    let system = mock();
    assert_eq!(
        install(&system, &config_path()).unwrap(),
        InstallOutcome::Installed,
    );
    assert_eq!(
        install(&system, &config_path()).unwrap(),
        InstallOutcome::AlreadyInstalled,
    );
}

/// A second install over an already-canonical entry must not touch the
/// file: goose's config is hand-maintained, and a rewrite would churn it
/// on every run.
#[test]
fn reinstalling_leaves_the_file_byte_identical() {
    let system = mock();
    let _outcome = install(&system, &config_path()).unwrap();
    let before = system.read_to_string(&config_path()).unwrap();
    assert_eq!(
        install(&system, &config_path()).unwrap(),
        InstallOutcome::AlreadyInstalled,
    );
    assert_eq!(system.read_to_string(&config_path()).unwrap(), before);
}

#[test]
fn install_rewrites_a_drifted_entry_in_place() {
    let system = seed(
        mock(),
        &entry_yaml("    type: stdio\n    name: remargin\n    cmd: /old/remargin\n"),
    );
    assert_eq!(
        install(&system, &config_path()).unwrap(),
        InstallOutcome::Installed,
    );
    assert_eq!(field(&entry_of(&system), "cmd"), Value::from(EXE));
}

// ---- 2. install beside unrelated config --------------------------------

/// goose reads its provider, its model, and every other extension from
/// this same file. An install that dropped any of it would cost the user
/// their whole goose setup, not just their remargin tools.
#[test]
fn install_preserves_sibling_extensions_and_unrelated_keys() {
    let system = seed(
        mock(),
        "GOOSE_TELEMETRY_ENABLED: false\nactive_provider: ollama\nproviders:\n  ollama:\n    \
         enabled: true\n    model: gemma4:31b\nextensions:\n  developer:\n    enabled: true\n    \
         type: builtin\n    name: developer\n",
    );
    let _outcome = install(&system, &config_path()).unwrap();

    let config = config_of(&system);
    assert_eq!(
        config.get(Value::from("active_provider")),
        Some(&Value::from("ollama")),
    );
    assert_eq!(
        config.get(Value::from("GOOSE_TELEMETRY_ENABLED")),
        Some(&Value::Bool(false)),
    );
    assert!(config.get(Value::from("providers")).is_some());

    let extensions = config
        .get(Value::from("extensions"))
        .unwrap()
        .as_mapping()
        .unwrap();
    assert!(
        extensions.get(Value::from("developer")).is_some(),
        "sibling extension dropped: {extensions:?}",
    );
    assert!(extensions.get(Value::from(EXTENSION_KEY)).is_some());
}

/// A config that does not parse is the one state that costs the user every
/// goose session, so install refuses it rather than rewriting from
/// scratch — the opposite of the guard plugin, which remargin owns whole.
#[test]
fn install_refuses_to_overwrite_an_unparseable_config() {
    let system = seed(mock(), "extensions: [ this is not\n");
    let err = install(&system, &config_path()).unwrap_err();
    assert!(
        err.to_string().contains("not valid YAML"),
        "error should name the parse fault: {err}",
    );
    assert_eq!(
        system.read_to_string(&config_path()).unwrap(),
        "extensions: [ this is not\n",
        "the unreadable config must survive untouched",
    );
}

/// goose leaves an empty `config.yaml` behind before its first
/// `configure`, and an empty YAML document parses as null.
#[test]
fn install_treats_an_empty_config_as_an_absence_to_fill() {
    let system = seed(mock(), "");
    assert_eq!(
        install(&system, &config_path()).unwrap(),
        InstallOutcome::Installed,
    );
    assert_eq!(
        entry_of(&system).get(Value::from("name")),
        Some(&Value::from(EXTENSION_NAME)),
    );
}

// ---- 3. uninstall ------------------------------------------------------

#[test]
fn uninstall_removes_only_remargins_entry() {
    let system = seed(
        mock(),
        "active_provider: ollama\nextensions:\n  developer:\n    enabled: true\n    type: \
         builtin\n",
    );
    let _outcome = install(&system, &config_path()).unwrap();
    assert_eq!(
        uninstall(&system, &config_path()).unwrap(),
        UninstallOutcome::Uninstalled,
    );

    let config = config_of(&system);
    assert_eq!(
        config.get(Value::from("active_provider")),
        Some(&Value::from("ollama")),
    );
    let extensions = config
        .get(Value::from("extensions"))
        .unwrap()
        .as_mapping()
        .unwrap();
    assert!(extensions.get(Value::from("developer")).is_some());
    assert!(extensions.get(Value::from(EXTENSION_KEY)).is_none());
}

#[test]
fn uninstall_is_a_no_op_when_absent() {
    let system = mock();
    assert_eq!(
        uninstall(&system, &config_path()).unwrap(),
        UninstallOutcome::NotInstalled,
    );

    let seeded = seed(mock(), "active_provider: ollama\n");
    assert_eq!(
        uninstall(&seeded, &config_path()).unwrap(),
        UninstallOutcome::NotInstalled,
    );
    assert_eq!(
        seeded.read_to_string(&config_path()).unwrap(),
        "active_provider: ollama\n",
        "a config with no remargin entry must not be rewritten",
    );
}

#[test]
fn uninstall_refuses_to_rewrite_an_unparseable_config() {
    let system = seed(mock(), "extensions: [ this is not\n");
    let err = uninstall(&system, &config_path()).unwrap_err();
    assert!(
        err.to_string().contains("not valid YAML"),
        "error should name the parse fault: {err}",
    );
}

// ---- 4. test verdicts --------------------------------------------------

#[test]
fn test_reports_installed_when_wired() {
    let system = mock();
    let _outcome = install(&system, &config_path()).unwrap();
    assert_eq!(
        test(&system, &config_path()).unwrap(),
        TestOutcome::Installed
    );
}

#[test]
fn test_reports_not_installed_when_absent() {
    assert_eq!(
        test(&mock(), &config_path()).unwrap(),
        TestOutcome::NotInstalled,
    );
    let seeded = seed(
        mock(),
        "active_provider: ollama\nextensions:\n  developer:\n    x: 1\n",
    );
    assert_eq!(
        test(&seeded, &config_path()).unwrap(),
        TestOutcome::NotInstalled,
    );
}

/// The failure mode observed on a live goose: the entry is there, the
/// binary is not, goose prints a warning and starts a session carrying
/// zero remargin tools. The guard still blocks and still names them.
#[test]
fn test_reports_broken_when_the_binary_is_gone() {
    let system = MockSystem::new()
        .with_current_exe(Path::new(EXE))
        .unwrap()
        .with_file(config_path(), wired_yaml().as_bytes())
        .unwrap();
    let reason = expect_broken(test(&system, &config_path()).unwrap());
    assert!(
        reason.contains("/opt/bin/remargin"),
        "reason should name the missing binary: {reason}",
    );
}

/// Every shape that leaves goose loading no remargin tools, each observed
/// on a live goose rather than derived from documentation.
#[test]
fn test_reports_broken_for_each_dead_end_shape() {
    let cases = [
        (
            "    enabled: false\n    type: stdio\n    name: remargin\n    cmd: \
             /opt/bin/remargin\n    args:\n    - mcp\n",
            "enabled: false",
        ),
        (
            "    enabled: true\n    type: stdio\n    name: remargin\n    args:\n    - mcp\n",
            "no `cmd`",
        ),
        (
            "    enabled: true\n    type: stdio\n    name: notremargin\n    cmd: \
             /opt/bin/remargin\n    args:\n    - mcp\n",
            "notremargin__",
        ),
        (
            "    enabled: true\n    type: builtin\n    name: remargin\n    cmd: \
             /opt/bin/remargin\n    args:\n    - mcp\n",
            "not `stdio`",
        ),
        (
            "    enabled: true\n    type: stdio\n    name: remargin\n    cmd: \
             /opt/bin/remargin\n    args: []\n",
            "does not pass `mcp`",
        ),
    ];
    for (fields, expected) in cases {
        let system = seed(mock(), &entry_yaml(fields));
        let reason = expect_broken(test(&system, &config_path()).unwrap());
        assert!(
            reason.contains(expected),
            "expected {expected:?} in the fault, got: {reason}",
        );
    }
}

/// A config goose cannot parse costs the user every session, so `test`
/// reports it rather than calling the extension merely absent.
#[test]
fn test_reports_broken_for_an_unparseable_config() {
    let system = seed(mock(), "extensions: [ this is not\n");
    let reason = expect_broken(test(&system, &config_path()).unwrap());
    assert!(
        reason.contains("not valid YAML"),
        "reason should name the parse fault: {reason}",
    );
}
