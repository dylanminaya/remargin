//! `edit` reports the same warn-tier style notes the create path reports:
//! text mode on stderr, `--json` as a structured array in the payload, and
//! nothing at all for a body that reads cleanly.

#[cfg(test)]
#[path = "cli_edit_advice/tests.rs"]
mod tests;
