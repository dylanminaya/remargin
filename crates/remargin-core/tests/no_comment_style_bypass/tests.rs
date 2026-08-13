use std::fs;
use std::path::{Path, PathBuf};

/// The gate's exact signature. Its two parameters are the whole of its
/// input surface; a third one would be the seam a bypass hangs off.
const GATE_SIGNATURE: &str = "pub fn gate(content: &str, author_type: &AuthorType) -> Result<()> {";

/// Tokens that must not appear in the gate's module. Each one is a way for
/// caller-supplied state to reach a decision that may only be made from the
/// body and the author type.
const BANNED_IN_GATE_MODULE: &[&str] = &[
    "ResolvedConfig",
    "env!",
    "option_env!",
    "std::env",
    "var_os",
];

fn gate_module() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/comment_style.rs")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_gate_takes_the_body_and_the_author_type_and_nothing_else() {
    let path = gate_module();
    let source = read(&path);

    assert!(
        source.contains(GATE_SIGNATURE),
        "{} no longer declares {GATE_SIGNATURE:?}. A new parameter is a new \
         way to turn the checks off; demote the check to the warn tier \
         instead.",
        path.display()
    );
}

#[test]
fn the_gate_module_reads_no_config_and_no_environment() {
    let path = gate_module();
    let source = read(&path);

    let mut hits: Vec<String> = Vec::new();
    for (line_no, line) in source.lines().enumerate() {
        for banned in BANNED_IN_GATE_MODULE {
            if line.contains(banned) {
                hits.push(format!("line {}: {}", line_no + 1, line.trim()));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "the comment gate must decide from the body and the author type \
         alone, so that no config key or environment variable can reach it. \
         Offenders in {}:\n{}",
        path.display(),
        hits.join("\n")
    );
}
