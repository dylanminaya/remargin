//! `remargin goose pretool` integration tests.
//!
//! Exercises the CLI subcommand against a real-filesystem temp realm.
//! goose's hook contract is the source of truth — every dispatch test
//! pipes an envelope into the binary and asserts on stdout, stderr, and
//! exit code, because goose fails open on anything it cannot read as a
//! block.

#[cfg(test)]
#[path = "cli_goose_pretool/tests.rs"]
mod tests;
