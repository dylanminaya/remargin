//! `remargin goose mcp` integration tests.
//!
//! Exercises the CLI subcommand against a real-filesystem temp home.
//! goose's config file is the source of truth — every lifecycle test reads
//! the YAML back and asserts on the entry goose would actually load,
//! because goose warns and continues past an extension it cannot start.

#[cfg(test)]
#[path = "cli_goose_mcp/tests.rs"]
mod tests;
