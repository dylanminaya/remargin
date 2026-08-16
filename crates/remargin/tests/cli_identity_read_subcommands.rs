//! The four read subcommands (`get`, `ls`, `comments`, `search`) accept
//! the `IdentityArgs` flag group, resolving the caller exactly as
//! `query` does. Callers that forward identity uniformly (the Obsidian
//! plugin) must never hit a clap "unexpected argument" rejection on a
//! read, and a passed identity must win over the walked one.

#[cfg(test)]
#[path = "cli_identity_read_subcommands/tests.rs"]
mod tests;
