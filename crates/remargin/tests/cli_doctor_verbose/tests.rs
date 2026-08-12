//! CLI integration tests for `remargin doctor --verbose`.
//!
//! Verifies that `--verbose` appends a `Checks:` section (hook-installed
//! verdict + inspected user/project settings paths) in both the clean and
//! findings cases, while non-verbose output is unchanged and `--json` is
//! unaffected.

use core::str;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use serde_json::json;
use tempfile::TempDir;

fn run_in(dir: &Path, args: &[&str]) -> Output {
    Command::cargo_bin("remargin")
        .unwrap()
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("XDG_CONFIG_HOME")
        .args(args)
        .output()
        .unwrap()
}

fn stdout_of(out: &Output) -> &str {
    str::from_utf8(&out.stdout).unwrap()
}

fn stderr_of(out: &Output) -> &str {
    str::from_utf8(&out.stderr).unwrap()
}

fn assert_status(out: &Output, expected: i32) {
    let actual = out.status.code();
    assert_eq!(
        actual,
        Some(expected),
        "remargin exited with {:?}\nstdout: {}\nstderr: {}",
        actual,
        stdout_of(out),
        stderr_of(out),
    );
}

/// The hook command a current install writes: the absolute path of the
/// binary under test plus the dispatch subcommand. A bare command name
/// would read as an older install and earn its own warning, which is not
/// what these fixtures are about.
fn hook_command(subcommand: &str) -> String {
    format!("{} {subcommand}", cargo_bin("remargin").display())
}

/// Build a JSON settings file containing both enforcement hooks — the
/// `PreToolUse` hook and the `SessionStart` guard — so `doctor` reports a
/// fully clean stack.
fn hook_settings_json() -> String {
    let v = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Read|Write|Edit|Bash|NotebookEdit",
                    "hooks": [
                        { "type": "command", "command": hook_command("claude pretool") }
                    ]
                }
            ],
            "SessionStart": [
                {
                    "hooks": [
                        { "type": "command", "command": hook_command("claude session-guard") }
                    ]
                }
            ]
        }
    });
    serde_json::to_string_pretty(&v).unwrap()
}

/// Helper: run doctor with an explicit --user-settings pointing at a
/// temporary file and `$HOME` pinned at the realm, so the test never
/// touches the real user home — neither for the settings file nor for the
/// home-anchored checks (the goose plugin root).
fn run_doctor_with_settings(realm: &Path, user_settings: &Path, extra_args: &[&str]) -> Output {
    let mut args = vec!["doctor", "--user-settings", user_settings.to_str().unwrap()];
    args.extend_from_slice(extra_args);
    run_in(realm, &args)
}

// ---- clean case ---------------------------------------------------------

/// Without `--verbose`, `doctor` in the clean case emits only the
/// one-liner "doctor: all checks passed" with no `Checks:` section.
#[test]
fn clean_plain_has_no_checks_section() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();

    let out = run_doctor_with_settings(realm.path(), &settings, &[]);
    assert_status(&out, 0);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("doctor: all checks passed"),
        "expected 'all checks passed' in:\n{stdout}",
    );
    assert!(
        !stdout.contains("Checks:"),
        "non-verbose output must not contain 'Checks:' section, got:\n{stdout}",
    );
}

/// With `--verbose`, `doctor` appends a `Checks:` section even in
/// the clean case.
#[test]
fn clean_verbose_appends_checks_section() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();

    let out = run_doctor_with_settings(realm.path(), &settings, &["--verbose"]);
    assert_status(&out, 0);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("doctor: all checks passed"),
        "expected 'all checks passed' in:\n{stdout}",
    );
    assert!(
        stdout.contains("Checks:"),
        "verbose output must contain 'Checks:' header, got:\n{stdout}",
    );
    assert!(
        stdout.contains("hook-installed: ok"),
        "verbose output must show hook-installed: ok, got:\n{stdout}",
    );
    assert!(
        stdout.contains("session-guard: ok"),
        "verbose output must show session-guard: ok, got:\n{stdout}",
    );
    assert!(
        stdout.contains("user-settings:"),
        "verbose output must show user-settings path, got:\n{stdout}",
    );
    assert!(
        stdout.contains("project-settings:"),
        "verbose output must show project-settings path, got:\n{stdout}",
    );
}

/// The verbose output differs from non-verbose output in the clean case.
#[test]
fn clean_verbose_differs_from_plain() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();

    let plain = run_doctor_with_settings(realm.path(), &settings, &[]);
    let verbose = run_doctor_with_settings(realm.path(), &settings, &["--verbose"]);

    assert_ne!(
        stdout_of(&plain),
        stdout_of(&verbose),
        "verbose and non-verbose output must differ in clean case",
    );
}

// ---- goose stack --------------------------------------------------------

/// The whole goose stack `remargin goose ... install` writes: both hook
/// entries and the MCP extension the guard redirects to, all naming a
/// binary that exists under `root`. The extension belongs here because the
/// guard's redirect is only a redirect when the session has those tools.
fn wire_goose_plugin(root: &Path) {
    let binary = root.join("bin/remargin");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, "binary").unwrap();

    let hooks = root.join(".agents/plugins/remargin-guard/hooks/hooks.json");
    fs::create_dir_all(hooks.parent().unwrap()).unwrap();
    let manifest = json!({
        "hooks": {
            "PreToolUse": [{ "hooks": [{
                "type": "command",
                "command": format!("{} goose pretool", binary.display()),
            }] }],
            "SessionStart": [{ "hooks": [{
                "type": "command",
                "command": format!("{} goose session-guard", binary.display()),
            }] }],
        }
    });
    fs::write(&hooks, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

    let config = root.join(".config/goose/config.yaml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        format!(
            "extensions:\n  remargin:\n    enabled: true\n    type: stdio\n    name: \
             remargin\n    cmd: {}\n    args:\n    - mcp\n",
            binary.display(),
        ),
    )
    .unwrap();
}

/// The realm has no goose installation, so the verbose section says nothing
/// about the goose stack.
#[test]
fn clean_verbose_omits_goose_lines_without_goose() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();

    let out = run_doctor_with_settings(realm.path(), &settings, &["--verbose"]);
    assert_status(&out, 0);
    let stdout = stdout_of(&out);
    assert!(
        !stdout.contains("goose-guard:")
            && !stdout.contains("goose-session-guard:")
            && !stdout.contains("goose-mcp:"),
        "no goose installed, so no goose verdict line, got:\n{stdout}",
    );
}

/// The same realm's `--json` leaves both verdict keys out entirely. The
/// generated contract renders an unset verdict as the absent form, so a
/// `null` here fails every consumer that validates against it.
#[test]
fn clean_json_omits_goose_verdict_keys_without_goose() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();

    let out = run_doctor_with_settings(realm.path(), &settings, &["--json"]);
    assert_status(&out, 0);
    let report: serde_json::Value = serde_json::from_str(stdout_of(&out)).unwrap();
    let fields = report.as_object().unwrap();
    assert!(
        !fields.contains_key("goose_guard_installed"),
        "unset verdict must be absent, not null, got:\n{report:#}",
    );
    assert!(
        !fields.contains_key("goose_session_guard_installed"),
        "unset verdict must be absent, not null, got:\n{report:#}",
    );
    assert!(
        !fields.contains_key("goose_mcp_installed"),
        "unset verdict must be absent, not null, got:\n{report:#}",
    );
}

/// A wired goose stack under the pinned `$HOME` renders one verdict line
/// per goose check, and `--json` carries the same verdicts.
#[test]
fn clean_verbose_reports_a_wired_goose_stack() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();
    wire_goose_plugin(realm.path());

    let out = run_doctor_with_settings(realm.path(), &settings, &["--verbose"]);
    assert_status(&out, 0);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("goose-guard: ok"),
        "verbose output must show goose-guard: ok, got:\n{stdout}",
    );
    assert!(
        stdout.contains("goose-session-guard: ok"),
        "verbose output must show goose-session-guard: ok, got:\n{stdout}",
    );
    assert!(
        stdout.contains("goose-mcp: ok"),
        "verbose output must show goose-mcp: ok, got:\n{stdout}",
    );

    let json_out = run_doctor_with_settings(realm.path(), &settings, &["--json"]);
    assert_status(&json_out, 0);
    let report: serde_json::Value = serde_json::from_str(stdout_of(&json_out)).unwrap();
    assert_eq!(report["goose_guard_installed"], json!(true));
    assert_eq!(report["goose_session_guard_installed"], json!(true));
    assert_eq!(report["goose_mcp_installed"], json!(true));
}

/// A goose installation with no guard plugin renders both verdicts as
/// missing beside the findings that name the repair.
#[test]
fn verbose_reports_an_unwired_goose_stack_as_missing() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();
    fs::create_dir_all(realm.path().join(".agents/plugins")).unwrap();

    let out = run_doctor_with_settings(realm.path(), &settings, &["--verbose"]);
    assert_status(&out, 1);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("goose-guard: missing"),
        "verbose output must show goose-guard: missing, got:\n{stdout}",
    );
    assert!(
        stdout.contains("goose-session-guard: missing"),
        "verbose output must show goose-session-guard: missing, got:\n{stdout}",
    );
}

// ---- findings case ------------------------------------------------------

/// Without `--verbose`, `doctor` in the findings case emits only the
/// finding lines with no `Checks:` section.
#[test]
fn findings_plain_has_no_checks_section() {
    let realm = TempDir::new().unwrap();
    // Point at a nonexistent user settings file (no hook installed anywhere).
    let fake_settings = realm.path().join("no_settings.json");

    let out = run_doctor_with_settings(realm.path(), &fake_settings, &[]);
    assert_status(&out, 1);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("[CRITICAL]"),
        "expected [CRITICAL] finding in:\n{stdout}",
    );
    assert!(
        !stdout.contains("Checks:"),
        "non-verbose findings output must not contain 'Checks:' section, got:\n{stdout}",
    );
}

/// With `--verbose`, `doctor` appends a `Checks:` section in the
/// findings case (hook-installed verdict = missing).
#[test]
fn findings_verbose_appends_checks_section() {
    let realm = TempDir::new().unwrap();
    let fake_settings = realm.path().join("no_settings.json");

    let out = run_doctor_with_settings(realm.path(), &fake_settings, &["--verbose"]);
    assert_status(&out, 1);
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("[CRITICAL]"),
        "expected [CRITICAL] finding in:\n{stdout}",
    );
    assert!(
        stdout.contains("Checks:"),
        "verbose findings output must contain 'Checks:' header, got:\n{stdout}",
    );
    assert!(
        stdout.contains("hook-installed: missing"),
        "verbose findings output must show hook-installed: missing, got:\n{stdout}",
    );
}

/// The verbose output differs from non-verbose output in the findings case.
#[test]
fn findings_verbose_differs_from_plain() {
    let realm = TempDir::new().unwrap();
    let fake_settings = realm.path().join("no_settings.json");

    let plain = run_doctor_with_settings(realm.path(), &fake_settings, &[]);
    let verbose = run_doctor_with_settings(realm.path(), &fake_settings, &["--verbose"]);

    assert_ne!(
        stdout_of(&plain),
        stdout_of(&verbose),
        "verbose and non-verbose output must differ in findings case",
    );
}

// ---- json case ----------------------------------------------------------

/// `--json` is unaffected by `--verbose` — it always emits the full
/// structured report and is identical with or without the flag.
#[test]
fn json_output_unaffected_by_verbose() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();

    let plain_json = run_doctor_with_settings(realm.path(), &settings, &["--json"]);
    let verbose_json = run_doctor_with_settings(realm.path(), &settings, &["--json", "--verbose"]);

    assert_status(&plain_json, 0);
    assert_status(&verbose_json, 0);

    // Both must be valid JSON. Strip elapsed_ms before comparing — it is a
    // wall-clock measurement that differs between two separate process runs.
    let mut plain_val: serde_json::Value = serde_json::from_str(stdout_of(&plain_json)).unwrap();
    let mut verbose_val: serde_json::Value =
        serde_json::from_str(stdout_of(&verbose_json)).unwrap();
    if let Some(obj) = plain_val.as_object_mut() {
        obj.remove("elapsed_ms");
    }
    if let Some(obj) = verbose_val.as_object_mut() {
        obj.remove("elapsed_ms");
    }

    assert_eq!(
        plain_val, verbose_val,
        "--json output must be identical with and without --verbose",
    );
}

// ---- --check selective-run case -----------------------------------------

/// Settings carrying only the `PreToolUse` hook — enforcement is wired but
/// the `SessionStart` guard is missing, so a run trips `SessionGuardMissing`
/// without short-circuiting on a missing hook.
fn pretool_only_settings_json() -> String {
    let v = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Read|Write|Edit|Bash|NotebookEdit",
                    "hooks": [
                        { "type": "command", "command": hook_command("claude pretool") }
                    ]
                }
            ]
        }
    });
    serde_json::to_string_pretty(&v).unwrap()
}

/// Collect the `kind` of every finding in a `doctor --json` payload.
fn finding_kinds(stdout: &str) -> Vec<String> {
    let val: serde_json::Value = serde_json::from_str(stdout).unwrap();
    val["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_owned())
        .collect()
}

/// A realm that trips both `SessionGuardMissing` (guard absent) and
/// `LeftoverProjectedRule` (stale `Bash(remargin *)` deny in
/// `settings.local.json`). Returns the user-settings path to pass through.
fn two_finding_realm(realm: &Path) -> PathBuf {
    let user_settings = realm.join("settings.json");
    fs::write(&user_settings, pretool_only_settings_json()).unwrap();
    fs::create_dir_all(realm.join(".claude")).unwrap();
    fs::write(
        realm.join(".claude/settings.local.json"),
        json!({ "permissions": { "deny": ["Bash(remargin *)"] } }).to_string(),
    )
    .unwrap();
    user_settings
}

/// `--check leftover-rules` reports only that check; the `session-guard`
/// finding the same realm carries under a full run is absent.
#[test]
fn check_scopes_run_to_selected() {
    let realm = TempDir::new().unwrap();
    let user_settings = two_finding_realm(realm.path());

    let full = run_doctor_with_settings(realm.path(), &user_settings, &["--json"]);
    assert_status(&full, 1);
    let full_kinds = finding_kinds(stdout_of(&full));
    assert!(
        full_kinds.iter().any(|k| k == "session_guard_missing")
            && full_kinds.iter().any(|k| k == "leftover_projected_rule"),
        "full run surfaces both findings: {full_kinds:?}",
    );

    let scoped = run_doctor_with_settings(
        realm.path(),
        &user_settings,
        &["--check", "leftover-rules", "--json"],
    );
    assert_status(&scoped, 1);
    assert_eq!(
        finding_kinds(stdout_of(&scoped)),
        vec![String::from("leftover_projected_rule")],
        "scoped run reports only the selected check",
    );
}

/// On a realm with no `PreToolUse` hook, `--check session-guard` exits 0
/// with no findings — the gate still short-circuits (`hook_installed` is
/// `false` and the guard check never runs), it just says nothing about a
/// check the caller did not select. Selecting `hook` reports the gate.
#[test]
fn check_scoped_run_on_hookless_realm_is_silent() {
    let realm = TempDir::new().unwrap();
    let user_settings = realm.path().join("settings.json");
    fs::write(&user_settings, json!({}).to_string()).unwrap();

    let scoped = run_doctor_with_settings(
        realm.path(),
        &user_settings,
        &["--check", "session-guard", "--json"],
    );
    assert_status(&scoped, 0);
    assert!(
        finding_kinds(stdout_of(&scoped)).is_empty(),
        "deselected gate reports nothing: {}",
        stdout_of(&scoped),
    );
    let report: serde_json::Value = serde_json::from_str(stdout_of(&scoped)).unwrap();
    assert_eq!(
        report["hook_installed"],
        json!(false),
        "the gate still ran: {report}",
    );

    let selected =
        run_doctor_with_settings(realm.path(), &user_settings, &["--check", "hook", "--json"]);
    assert_status(&selected, 1);
    assert_eq!(
        finding_kinds(stdout_of(&selected)),
        vec![String::from("hook_missing")],
        "selecting hook reports the gate finding",
    );
}

/// An unknown `--check` name is a hard CLI error naming the bad slug and
/// listing the valid ones — never a silent empty run.
#[test]
fn check_unknown_name_errors() {
    let realm = TempDir::new().unwrap();
    let settings = realm.path().join("settings.json");
    fs::write(&settings, hook_settings_json()).unwrap();

    let out = run_doctor_with_settings(realm.path(), &settings, &["--check", "bogus"]);
    assert!(
        !out.status.success(),
        "unknown check must exit non-zero, got:\nstdout: {}\nstderr: {}",
        stdout_of(&out),
        stderr_of(&out),
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("unknown check `bogus`") && stderr.contains("session-guard"),
        "stderr must name the bad slug and list valid ones, got:\n{stderr}",
    );
}
