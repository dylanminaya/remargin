//! `remargin goose session-guard` integration tests.
//!
//! Exercises the CLI subcommand against a real-filesystem temp realm. The
//! guard is diagnostic-only — goose gives `SessionStart` no blocking
//! decision — so every dispatch test asserts on what lands on stdout and
//! on the exit code staying 0, and every lifecycle test asserts that the
//! `PreToolUse` entry sharing the plugin is left alone.

#[cfg(test)]
#[path = "cli_goose_session_guard/tests.rs"]
mod tests;
