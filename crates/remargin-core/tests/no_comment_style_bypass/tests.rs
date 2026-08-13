use std::fs;
use std::path::{Path, PathBuf};

/// The gate's exact signature. Its two parameters are the whole of its
/// input surface; a third one would be the seam a bypass hangs off.
const GATE_SIGNATURE: &str = "pub fn gate(content: &str, author_type: &AuthorType) -> Result<()> {";

/// The edit gate's exact signature. The old body, the new body and the
/// author type are the whole of its input surface; a fourth parameter would
/// be the same seam under another name.
const EDIT_GATE_SIGNATURE: &str = "pub fn gate_edit(old_content: &str, new_content: &str, author_type: &AuthorType) -> Result<()> {";

/// The edit path's entry point, and the call it has to carry. An edit that
/// reaches a write without it is how a gated body gets rewritten into an
/// ungated one.
const EDIT_ENTRY_POINT: &str = "pub fn edit_comment(";
const EDIT_GATE_CALL: &str = "comment_style::gate_edit(";

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

fn operations_module() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/operations.rs")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// `source` from the line opening `signature` through the function's closing
/// brace at column 0, or `None` when nothing declares it any more.
fn function_body<'src>(source: &'src str, signature: &str) -> Option<&'src str> {
    let start = source.find(signature)?;
    let rest = &source[start..];
    let end = rest.find("\n}\n").map_or(rest.len(), |idx| idx + 3);
    Some(&rest[..end])
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
fn the_edit_gate_takes_both_bodies_and_the_author_type_and_nothing_else() {
    let path = gate_module();
    let source = read(&path);

    assert!(
        source.contains(EDIT_GATE_SIGNATURE),
        "{} no longer declares {EDIT_GATE_SIGNATURE:?}. A new parameter is a \
         new way to turn the checks off; demote the check to the warn tier \
         instead.",
        path.display()
    );
}

#[test]
fn the_edit_path_runs_the_edit_gate() {
    let path = operations_module();
    let source = read(&path);

    assert!(
        function_body(&source, EDIT_ENTRY_POINT).is_some_and(|body| body.contains(EDIT_GATE_CALL)),
        "{} no longer reaches {EDIT_GATE_CALL:?} from {EDIT_ENTRY_POINT:?}, \
         so a body the gate refused on creation can be written through the \
         edit path instead.",
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
