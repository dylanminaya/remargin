use std::path::Path;

use os_shim::mock::MockSystem;
use serde_json::json;

use super::{GuardDiagnostic, GuardDiagnosticInner, GuardOutcome, session_guard};
use crate::permissions::pretool_install::{HOOK_MATCHER, HOOK_SUBCOMMAND};

const EXE: &str = "/opt/bin/remargin";

/// A mock whose `PATH` contains a directory holding a `remargin` file, so
/// the on-PATH check passes and only the config check can fail.
fn mock_with_remargin_on_path() -> MockSystem {
    MockSystem::new()
        .with_dir(Path::new("/usr/bin"))
        .unwrap()
        .with_file(Path::new("/usr/bin/remargin"), b"")
        .unwrap()
        .with_env("PATH", "/usr/bin")
        .unwrap()
}

/// Project-scope settings declaring a `PreToolUse` entry that runs
/// `command`, under a realm at `/r`.
fn realm_with_hook_command(system: MockSystem, command: &str) -> MockSystem {
    let settings = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": HOOK_MATCHER,
                    "hooks": [ { "type": "command", "command": command } ]
                }
            ]
        }
    });
    system
        .with_dir(Path::new("/r"))
        .unwrap()
        .with_file(
            Path::new("/r/.claude/settings.json"),
            serde_json::to_string_pretty(&settings).unwrap().as_bytes(),
        )
        .unwrap()
}

/// Destructure a `Fail` outcome without a `panic!` (denied by clippy). The
/// `matches!` assert carries the diagnostic; the else arm is unreachable.
fn expect_fail(outcome: GuardOutcome) -> GuardDiagnostic {
    assert!(
        matches!(outcome, GuardOutcome::Fail(_)),
        "expected GuardOutcome::Fail, got {outcome:?}",
    );
    let GuardOutcome::Fail(diagnostic) = outcome else {
        return GuardDiagnostic {
            hook_specific_output: GuardDiagnosticInner {
                additional_context: String::new(),
                hook_event_name: "SessionStart",
            },
            system_message: String::new(),
        };
    };
    diagnostic
}

/// Case 4: an unparseable realm `.remargin.yaml` above cwd → the guard
/// fails and surfaces a diagnostic naming the parse failure.
#[test]
fn unparseable_realm_config_fails() {
    let system = mock_with_remargin_on_path()
        .with_dir(Path::new("/r"))
        .unwrap()
        .with_file(Path::new("/r/.remargin.yaml"), b": : not valid yaml : :")
        .unwrap();

    let diag = expect_fail(session_guard(&system, Path::new("/r")));
    assert_eq!(diag.hook_specific_output.hook_event_name, "SessionStart");
    assert!(
        diag.hook_specific_output
            .additional_context
            .contains(".remargin.yaml"),
        "diagnostic should name the config: {}",
        diag.hook_specific_output.additional_context,
    );
    assert!(
        diag.system_message.contains("remargin doctor"),
        "system message should point at doctor: {}",
        diag.system_message,
    );
}

/// Binary on PATH + parseable config → the session proceeds clean.
#[test]
fn binary_present_and_config_parses_is_ok() {
    let system = mock_with_remargin_on_path()
        .with_dir(Path::new("/r"))
        .unwrap()
        .with_file(
            Path::new("/r/.remargin.yaml"),
            b"identity: alice\ntype: human\n",
        )
        .unwrap();

    assert_eq!(session_guard(&system, Path::new("/r")), GuardOutcome::Ok);
}

/// No `.remargin.yaml` on the walk is not a failure — an absent realm
/// config parses vacuously.
#[test]
fn no_realm_config_is_ok_when_binary_present() {
    let system = mock_with_remargin_on_path()
        .with_dir(Path::new("/r"))
        .unwrap();

    assert_eq!(session_guard(&system, Path::new("/r")), GuardOutcome::Ok);
}

/// `remargin` absent from every `PATH` entry → the guard fails and the
/// diagnostic explains the fail-open (exit 127, non-blocking) risk.
#[test]
fn binary_not_on_path_fails() {
    let system = MockSystem::new()
        .with_dir(Path::new("/usr/bin"))
        .unwrap()
        .with_env("PATH", "/usr/bin")
        .unwrap()
        .with_dir(Path::new("/r"))
        .unwrap();

    let diag = expect_fail(session_guard(&system, Path::new("/r")));
    assert!(
        diag.hook_specific_output
            .additional_context
            .contains("PATH"),
        "diagnostic should mention PATH: {}",
        diag.hook_specific_output.additional_context,
    );
}

/// A missing `PATH` variable is treated as "not resolvable" → failure.
#[test]
fn missing_path_var_fails() {
    let system = MockSystem::new().with_dir(Path::new("/r")).unwrap();

    assert!(matches!(
        session_guard(&system, Path::new("/r")),
        GuardOutcome::Fail(_)
    ));
}

/// The installed entry names an absolute binary that is on disk, so the
/// hook will spawn — `PATH` says nothing about it, and an empty `PATH` is
/// no longer a failure.
#[test]
fn absolute_hook_command_is_ok_without_the_binary_on_path() {
    let system = realm_with_hook_command(
        MockSystem::new()
            .with_file(Path::new(EXE), b"binary")
            .unwrap()
            .with_env("PATH", "/usr/bin")
            .unwrap(),
        &format!("{EXE} {HOOK_SUBCOMMAND}"),
    );

    assert_eq!(session_guard(&system, Path::new("/r")), GuardOutcome::Ok);
}

/// The installed entry names an absolute binary that is gone: the hook
/// cannot spawn, so the guard fails and names the binary — even though
/// another `remargin` does resolve on `PATH`.
#[test]
fn stale_absolute_hook_command_fails_and_names_the_binary() {
    let system = realm_with_hook_command(
        mock_with_remargin_on_path(),
        &format!("/gone/remargin {HOOK_SUBCOMMAND}"),
    );

    let diag = expect_fail(session_guard(&system, Path::new("/r")));
    let context = diag.hook_specific_output.additional_context;
    assert!(
        context.contains("/gone/remargin") && context.contains("does not exist"),
        "diagnostic should name the vanished binary: {context}",
    );
}

/// An entry an older install left behind resolves through `PATH`, so that
/// is what the guard checks it against — present here, so the session is
/// clean.
#[test]
fn path_relative_hook_command_falls_back_to_the_path_probe() {
    let legacy = format!("remargin {HOOK_SUBCOMMAND}");
    let clean = realm_with_hook_command(mock_with_remargin_on_path(), &legacy);
    assert_eq!(session_guard(&clean, Path::new("/r")), GuardOutcome::Ok);

    let broken = realm_with_hook_command(
        MockSystem::new().with_env("PATH", "/usr/bin").unwrap(),
        &legacy,
    );
    let diag = expect_fail(session_guard(&broken, Path::new("/r")));
    assert!(
        diag.hook_specific_output
            .additional_context
            .contains("PATH"),
        "diagnostic should mention PATH: {}",
        diag.hook_specific_output.additional_context,
    );
}
