//! Pins removal of unreachable NIF argument-vector arity guards.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_sources(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut pending = vec![root.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    Ok(sources)
}

fn compact_with_offsets(source: &str) -> (String, Vec<usize>) {
    let mut compact = String::with_capacity(source.len());
    let mut offsets = Vec::with_capacity(source.len());
    for (offset, character) in source.char_indices() {
        if !character.is_whitespace() {
            compact.push(character);
            offsets.extend(std::iter::repeat_n(offset, character.len_utf8()));
        }
    }
    (compact, offsets)
}

fn line_number(source: &str, byte_offset: usize) -> usize {
    source[..byte_offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

#[test]
fn no_unreachable_nif_arity_guard_remains() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut guards = Vec::new();
    for path in rust_sources(&root)? {
        let source = fs::read_to_string(&path)?;
        let (compact, offsets) = compact_with_offsets(&source);
        for needle in ["args.len()>255", "args.len()>u8::MAX.into()"] {
            for (offset, _) in compact.match_indices(needle) {
                guards.push(format!(
                    "{}:{}: {needle}",
                    path.display(),
                    line_number(&source, offsets[offset])
                ));
            }
        }
    }
    guards.sort();

    assert!(
        guards.is_empty(),
        "NIF arity is already a u8 before args is built; found {} unreachable guards:\n{}",
        guards.len(),
        guards.join("\n")
    );
    Ok(())
}
