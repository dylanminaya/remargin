//! `batch` reports the same warn-tier style notes the single-comment path
//! reports, scoped to the operation whose body earned them: text mode on
//! stderr, `--json` as a structured array in the payload.

#[cfg(test)]
#[path = "cli_batch_advice/tests.rs"]
mod tests;
