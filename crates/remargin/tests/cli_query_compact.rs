//! End-to-end CLI tests for `remargin query --json --compact`.
//!
//! Proves the compact columnar payload (matching the MCP `query` contract)
//! is emitted minified, that `--include-integrity` widens the rows, that
//! the verbose `--json` payload stays unchanged, that `base_path` joined
//! with a result path reconstructs the queried file, and that the clap
//! `requires` chain (`--include-integrity` -> `--compact` -> `--json`) is
//! enforced.

#[cfg(test)]
#[path = "cli_query_compact/tests.rs"]
mod tests;
