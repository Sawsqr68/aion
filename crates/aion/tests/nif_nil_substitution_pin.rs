//! Structural pin preventing NIF allocation failures from becoming success-domain values.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Offense {
    path: PathBuf,
    line: usize,
    property: &'static str,
}

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

fn line_number(source: &str, byte_offset: usize) -> usize {
    source[..byte_offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
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

fn record_matches(
    offenses: &mut Vec<Offense>,
    path: &Path,
    source: &str,
    compact: &str,
    offsets: &[usize],
    needle: &str,
    property: &'static str,
) {
    for (compact_offset, _) in compact.match_indices(needle) {
        offenses.push(Offense {
            path: path.to_path_buf(),
            line: line_number(source, offsets[compact_offset]),
            property,
        });
    }
}

fn option_term_signatures(
    offenses: &mut Vec<Offense>,
    path: &Path,
    source: &str,
    compact: &str,
    offsets: &[usize],
) {
    let mut search_from = 0;
    while let Some(relative_fn) = compact[search_from..].find("fn") {
        let fn_offset = search_from + relative_fn;
        let Some(relative_body) = compact[fn_offset..].find('{') else {
            break;
        };
        let body_offset = fn_offset + relative_body;
        let signature = &compact[fn_offset..body_offset];
        if signature.contains("->Option<Term>") {
            offenses.push(Offense {
                path: path.to_path_buf(),
                line: line_number(source, offsets[fn_offset]),
                property: "fn returning Option<Term>",
            });
        }
        search_from = body_offset + 1;
    }
}

fn scan_source(path: &Path, source: &str) -> Vec<Offense> {
    let (compact, offsets) = compact_with_offsets(source);
    let mut offenses = Vec::new();
    option_term_signatures(&mut offenses, path, source, &compact, &offsets);
    for needle in [
        "unwrap_or(Term::NIL)",
        "unwrap_or_else(||Term::NIL)",
        "None=>Term::NIL",
        "Err(_)=>Term::NIL",
    ] {
        record_matches(
            &mut offenses,
            path,
            source,
            &compact,
            &offsets,
            needle,
            "Term::NIL in fallback position",
        );
    }
    for allocator in ["alloc_binary(", "alloc_tuple("] {
        let mut search_from = 0;
        while let Some(relative_alloc) = compact[search_from..].find(allocator) {
            let alloc_offset = search_from + relative_alloc;
            let arguments_start = alloc_offset + allocator.len() - 1;
            let mut depth = 0_usize;
            let mut call_end = None;
            for (relative, byte) in compact[arguments_start..].bytes().enumerate() {
                match byte {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            call_end = Some(arguments_start + relative + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if call_end.is_some_and(|end| compact[end..].starts_with(".ok()")) {
                offenses.push(Offense {
                    path: path.to_path_buf(),
                    line: line_number(source, offsets[alloc_offset]),
                    property: "allocation error discarded with .ok()",
                });
            }
            search_from = alloc_offset + allocator.len();
        }
    }
    offenses
}

#[test]
fn no_allocation_failure_path_returns_a_success_domain_value()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenses = Vec::new();
    for path in rust_sources(&root)? {
        let source = fs::read_to_string(&path)?;
        offenses.extend(scan_source(&path, &source));
    }
    offenses.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.property.cmp(right.property))
    });

    let listing = offenses
        .iter()
        .map(|offense| {
            format!(
                "{}:{}: {}",
                offense.path.display(),
                offense.line,
                offense.property
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        offenses.is_empty(),
        "no allocation-failure path may return a success-domain value; found {} offenses:\n{}",
        offenses.len(),
        listing
    );
    Ok(())
}
