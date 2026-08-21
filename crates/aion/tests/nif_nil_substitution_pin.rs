//! Structural pin preventing NIF allocation failures from becoming success-domain values.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Offense {
    path: PathBuf,
    line: usize,
    property: &'static str,
}

#[derive(Debug)]
struct NilPosition {
    path: PathBuf,
    line: usize,
    classification: &'static str,
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

fn previous_non_whitespace(source: &str, before: usize) -> Option<usize> {
    source[..before]
        .char_indices()
        .rev()
        .find_map(|(offset, character)| (!character.is_whitespace()).then_some(offset))
}

fn identifier_before_call(source: &str, open_parenthesis: usize) -> Option<&str> {
    if source.as_bytes().get(open_parenthesis) != Some(&b'(') {
        return None;
    }
    let end = previous_non_whitespace(source, open_parenthesis)? + 1;
    let mut start = end;
    while start > 0 {
        let byte = source.as_bytes()[start - 1];
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            start -= 1;
        } else {
            break;
        }
    }
    (start < end).then_some(&source[start..end])
}

fn direct_payload_method(source: &str, term_offset: usize) -> Option<&str> {
    let open_parenthesis = previous_non_whitespace(source, term_offset)?;
    identifier_before_call(source, open_parenthesis)
}

fn closure_payload_method(source: &str, term_offset: usize) -> Option<&str> {
    let mut body_start = previous_non_whitespace(source, term_offset)?;
    if source.as_bytes()[body_start] == b'{' {
        body_start = previous_non_whitespace(source, body_start)?;
    }
    if source.as_bytes()[body_start] != b'|' {
        return None;
    }
    let binder_start = source[..body_start].rfind('|')?;
    let open_parenthesis = previous_non_whitespace(source, binder_start)?;
    identifier_before_call(source, open_parenthesis)
}

fn enclosing_block_start(source: &str, term_offset: usize) -> Option<usize> {
    let mut blocks = Vec::new();
    for (offset, byte) in source[..term_offset].bytes().enumerate() {
        match byte {
            b'{' => blocks.push(offset),
            b'}' => {
                blocks.pop();
            }
            _ => {}
        }
    }
    blocks.pop()
}

fn block_parked_before(source: &str, term_offset: usize) -> bool {
    let Some(block_start) = enclosing_block_start(source, term_offset) else {
        return false;
    };
    let (prefix, _) = compact_with_offsets(&source[block_start + 1..term_offset]);
    prefix.contains("request_suspend(") || prefix.contains("set_continuation_trampoline(")
}

fn is_success_domain_return(source: &str, term_offset: usize) -> bool {
    if direct_payload_method(source, term_offset) == Some("Ok") {
        return true;
    }
    let Some(end) = previous_non_whitespace(source, term_offset).map(|offset| offset + 1) else {
        return false;
    };
    source[..end].ends_with("return")
}

fn nil_classification(source: &str, term_offset: usize) -> &'static str {
    match direct_payload_method(source, term_offset) {
        Some("Err") => "PERMITTED P1 — Err(Term::NIL) error-domain construction",
        Some("ok_or") => "PERMITTED P1 — ok_or(Term::NIL) error-domain construction",
        _ => match closure_payload_method(source, term_offset) {
            Some("map_err") => "PERMITTED P1 — map_err(|..| Term::NIL) error-domain construction",
            Some("ok_or_else") => {
                "PERMITTED P1 — ok_or_else(|..| Term::NIL) error-domain construction"
            }
            _ if is_success_domain_return(source, term_offset)
                && block_parked_before(source, term_offset) =>
            {
                "PERMITTED P2 — success-domain return after a park in the enclosing block"
            }
            _ => "OFFENSE — Term::NIL is outside every explicitly permitted position",
        },
    }
}

fn last_else_keyword(source: &str, before: usize) -> Option<usize> {
    source[..before]
        .rmatch_indices("else")
        .find_map(|(offset, _)| {
            let separated_before = offset == 0
                || !source.as_bytes()[offset - 1].is_ascii_alphanumeric()
                    && source.as_bytes()[offset - 1] != b'_';
            let after = offset + "else".len();
            let opens_block = previous_non_whitespace(source, before)
                .is_some_and(|_| source[after..before].trim_start().starts_with('{'));
            (separated_before && opens_block).then_some(offset)
        })
}

fn offense_anchor(source: &str, term_offset: usize) -> usize {
    let prefix = &source[..term_offset];
    let branch_offset = [prefix.rfind("=>"), last_else_keyword(source, term_offset)]
        .into_iter()
        .flatten()
        .max();
    let Some(branch_offset) = branch_offset else {
        return term_offset;
    };
    source[..branch_offset]
        .rfind('\n')
        .map_or(0, |newline| newline + 1)
}

fn scan_source(path: &Path, source: &str) -> (Vec<Offense>, Vec<NilPosition>) {
    let (compact, offsets) = compact_with_offsets(source);
    let mut offenses = Vec::new();
    let mut positions = Vec::new();
    option_term_signatures(&mut offenses, path, source, &compact, &offsets);
    for (compact_offset, _) in compact.match_indices("Term::NIL") {
        let source_offset = offsets[compact_offset];
        let line = line_number(source, source_offset);
        let classification = nil_classification(source, source_offset);
        positions.push(NilPosition {
            path: path.to_path_buf(),
            line,
            classification,
        });
        if classification.starts_with("OFFENSE") {
            offenses.push(Offense {
                path: path.to_path_buf(),
                line: line_number(source, offense_anchor(source, source_offset)),
                property: "Term::NIL outside explicitly permitted position",
            });
        }
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
    (offenses, positions)
}

fn display_path<'a>(path: &'a Path, workspace_root: &Path) -> &'a Path {
    match path.strip_prefix(workspace_root) {
        Ok(relative) => relative,
        Err(_) => path,
    }
}

#[test]
fn no_allocation_failure_path_returns_a_success_domain_value()
-> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join("src");
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("CARGO_MANIFEST_DIR must be a crate nested under the workspace crates directory")?;
    let mut offenses = Vec::new();
    let mut positions = Vec::new();
    for path in rust_sources(&root)? {
        let source = fs::read_to_string(&path)?;
        let (source_offenses, source_positions) = scan_source(&path, &source);
        offenses.extend(source_offenses);
        positions.extend(source_positions);
    }
    offenses.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.property.cmp(right.property))
    });
    positions.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.classification.cmp(right.classification))
    });

    println!(
        "Term::NIL position classification (the permitted list is a floor, not the property):"
    );
    for position in &positions {
        println!(
            "{}:{}: {}",
            display_path(&position.path, workspace_root).display(),
            position.line,
            position.classification
        );
    }

    let listing = offenses
        .iter()
        .map(|offense| {
            format!(
                "{}:{}: {}",
                display_path(&offense.path, workspace_root).display(),
                offense.line,
                offense.property
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        offenses.is_empty(),
        "no allocation-failure path may return a success-domain value.\n\
\n\
This pin enforces that property by PERMISSION, not by enumeration. Every textual\n\
`Term::NIL` under crates/aion/src is an offense unless it sits in a position this\n\
pin explicitly permits:\n\
\n\
  * error-domain construction — `Err(`, `ok_or(`, `map_err(|..|`,\n\
    `ok_or_else(|..|`. These are raise reasons, not success-domain values.\n\
  * a success-domain return in a block that has already parked the process —\n\
    `request_suspend(`, `set_continuation_trampoline(`. The NIL is never observed\n\
    by workflow code; the native is re-invoked from the top on wake.\n\
\n\
Anything else trips, including spellings nobody has thought of. The permitted\n\
list is a floor: if your new site is legitimate, widen it deliberately and say\n\
why. Do not widen it to make a red go away.\n\
\n\
WHAT THIS PIN STILL CANNOT SEE — a green run means this much and no more:\n\
\n\
  * indirection: a `Term::NIL` reaching a fallback position through a helper call\n\
    carries no token at that site and is invisible. Text cannot follow a call.\n\
  * tokenless production: any expression evaluating to NIL without naming it.\n\
    `.unwrap_or_default()` is the obvious one; it does not compile today only\n\
    because beamr's `Term` has no `Default` impl (beamr src/term/mod.rs:61-63\n\
    derives Copy, Clone, Debug). That is a property of a pinned dependency, not\n\
    of this pin — if beamr adds one, this pin will not see it.\n\
  * park coverage: this pin sees that a park call precedes the NIL in the same\n\
    block. It cannot see whether that park covers the path that returns.\n\
  * scope: only crates/aion/src is scanned. tests/, sibling crates, include!d and\n\
    macro-expanded source are outside it.\n\
  * runtime origin: a NIL arriving from beamr rather than written here.\n\
\n\
found {} offenses:\n{}",
        offenses.len(),
        listing
    );
    Ok(())
}
