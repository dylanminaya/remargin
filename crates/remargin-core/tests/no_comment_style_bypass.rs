//! Structural guard: the comment-body gate must stay unbypassable.
//!
//! A check that can be switched off is a check that will be switched off,
//! forty turns into a session, by the author it was written to stop. So the
//! gate takes the body and the author type and nothing else: with no config,
//! no environment and no third parameter reaching it, there is nowhere for a
//! flag, a config key or an environment variable to attach. The edit gate is
//! held to the same shape, and the edit path has to call it — rewriting a
//! body is the other way to land one the gate would have refused.
//!
//! This test reads the source rather than the behaviour, because the thing
//! being asserted is the absence of a seam, and absences do not show up in a
//! run. A check that genuinely fires wrongly is demoted to the warn tier —
//! never given an escape hatch.

#[cfg(test)]
#[path = "no_comment_style_bypass/tests.rs"]
mod tests;
