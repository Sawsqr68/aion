//! The editors must colour the whole language, and the Zed pin must name a
//! mirror commit that carries the current grammar.
//!
//! AWL's highlighting is copied into three places that no test compared:
//! `tools/tree-sitter-awl/queries/` is authoritative, `editors/zed-awl/` and
//! `editors/nvim-awl/` each vendor their own adaptation, and Zed additionally
//! fetches the compiled parser from a standalone mirror repository by revision.
//! Every copy looked right in isolation, so nothing failed while all of them
//! fell behind.
//!
//! Measured 2026-07-31: both editor copies were missing `raw_string`,
//! `const_declaration`, `subflow_declaration`, `json_body` and `schema_body`,
//! and both stopped the combinator list at `count` — the entire B1/B2 flow
//! vocabulary, in an author's own editor, rendered as ordinary text. The Zed
//! `rev` pin was 436 commits behind on top of that.
//!
//! The invariant asserted here is NODE COVERAGE, not capture names. Zed writes
//! `@comment.doc` where Neovim writes `@comment.documentation`, and `@number`
//! where the authoritative query distinguishes `@number.float`; those are each
//! editor's theme vocabulary and are none of this gate's business. What is not
//! negotiable is that a node the authoritative query gives a colour to is given
//! SOME colour by every copy. An editor may capture more than the authoritative
//! set — extra folds are a UX choice — but never less.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use aion_awl::builtins::BUILTIN_TYPES;
use sha2::{Digest, Sha256};

/// Repository root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || unreachable!("crates/aion-awl always has two ancestors"),
            Path::to_path_buf,
        )
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| unreachable!("{} is not readable: {error}", path.display()))
}

/// Every node type a tree-sitter query file matches on.
///
/// Comment lines are stripped first: the authoritative highlight query explains
/// the `json { … }` door in prose, and prose is not a pattern. Predicates
/// (`(#match? …)`) and capture references (`@name`) are not node types and do
/// not start with a lowercase letter after `(`, so they fall out naturally.
fn nodes_matched(query: &str) -> BTreeSet<String> {
    let mut nodes = BTreeSet::new();
    for line in query
        .lines()
        .map(|line| line.split(';').next().unwrap_or(""))
    {
        let mut rest = line;
        while let Some(open) = rest.find('(') {
            rest = &rest[open + 1..];
            let name: String = rest
                .chars()
                .take_while(|character| character.is_ascii_lowercase() || *character == '_')
                .collect();
            // A node name must run to a delimiter; `(#match?` and `(` followed
            // by anything else contribute nothing.
            let ends_cleanly = rest[name.len()..]
                .chars()
                .next()
                .is_none_or(|next| next == ')' || next == ' ' || next == '\n');
            if !name.is_empty() && ends_cleanly {
                nodes.insert(name);
            }
        }
    }
    nodes
}

/// The authoritative query and the editor copies that must cover it.
const QUERY_COVERAGE: [(&str, &[&str]); 3] = [
    (
        "tools/tree-sitter-awl/queries/highlights.scm",
        &[
            "editors/zed-awl/languages/awl/highlights.scm",
            "editors/nvim-awl/queries/awl/highlights.scm",
        ],
    ),
    (
        "tools/tree-sitter-awl/queries/indents.scm",
        &[
            "editors/zed-awl/languages/awl/indents.scm",
            "editors/nvim-awl/queries/awl/indents.scm",
        ],
    ),
    (
        // Zed ships no fold query for AWL, so it carries no fold obligation.
        "tools/tree-sitter-awl/queries/folds.scm",
        &["editors/nvim-awl/queries/awl/folds.scm"],
    ),
];

/// Every query file in the repository, authoritative and vendored alike.
const ALL_QUERIES: [&str; 9] = [
    "tools/tree-sitter-awl/queries/highlights.scm",
    "tools/tree-sitter-awl/queries/indents.scm",
    "tools/tree-sitter-awl/queries/folds.scm",
    "editors/zed-awl/languages/awl/highlights.scm",
    "editors/zed-awl/languages/awl/indents.scm",
    "editors/zed-awl/languages/awl/outline.scm",
    "editors/nvim-awl/queries/awl/highlights.scm",
    "editors/nvim-awl/queries/awl/indents.scm",
    "editors/nvim-awl/queries/awl/folds.scm",
];

/// No editor colours less of the language than the authoritative query does.
#[test]
fn every_editor_covers_every_node_the_authoritative_query_colours() {
    for (authority, copies) in QUERY_COVERAGE {
        let expected = nodes_matched(&read(authority));
        assert!(
            expected.len() > 3,
            "{authority} yielded only {} node types, so the comparisons below would pass \
             vacuously — the extraction is broken, not the editors",
            expected.len()
        );

        for copy in copies {
            let covered = nodes_matched(&read(copy));
            let missing: Vec<&String> = expected.difference(&covered).collect();
            assert!(
                missing.is_empty(),
                "{copy} does not match these node types, which {authority} gives a capture to: \
                 {missing:?}\nThe author sees them as plain text in that editor. Add a pattern \
                 for each, using that editor's own capture vocabulary — the capture NAME is free, \
                 the node coverage is not."
            );
        }
    }
}

/// Every node named in any query file is a node this grammar actually produces.
///
/// A pattern naming a node that no longer exists matches nothing and reports
/// nothing; it is a colour that silently stopped applying. This is the reverse
/// direction of the test above and catches a rename on the grammar side.
#[test]
fn no_query_names_a_node_the_grammar_does_not_produce() {
    let node_types = read("tools/tree-sitter-awl/src/node-types.json");
    // Field names are legal in a pattern too (`name: (identifier)`), and they
    // appear in node-types.json inside each node's "fields" object, so a single
    // substring search over the file covers both without parsing it.
    for query in ALL_QUERIES {
        let unknown: Vec<String> = nodes_matched(&read(query))
            .into_iter()
            .filter(|node| !node_types.contains(&format!("\"{node}\"")))
            .collect();
        assert!(
            unknown.is_empty(),
            "{query} matches on node types absent from \
             tools/tree-sitter-awl/src/node-types.json: {unknown:?}\nThose patterns match nothing \
             and colour nothing. Either the grammar renamed them and the query was not followed \
             through, or the names are typos."
        );
    }
}

/// The tree-sitter builtin token rule stays aligned with AWL's builtin types.
#[test]
fn grammar_builtin_types_match_the_awl_authority() {
    let grammar = read("tools/tree-sitter-awl/grammar.js");
    let rules: Vec<&str> = grammar
        .lines()
        .filter(|line| line.trim_start().starts_with("builtin_type:"))
        .collect();
    let [rule] = rules.as_slice() else {
        unreachable!(
            "tools/tree-sitter-awl/grammar.js must contain exactly one `builtin_type:` rule; found {}",
            rules.len()
        )
    };
    let grammar_builtins: Vec<&str> = rule
        .split('\'')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 1).then_some(part))
        .collect();

    assert_eq!(
        grammar_builtins.as_slice(),
        BUILTIN_TYPES.as_slice(),
        "tools/tree-sitter-awl/grammar.js builtin_type names drifted from \
         crates/aion-awl/src/builtins.rs BUILTIN_TYPES; grammar={grammar_builtins:?}, \
         authority={BUILTIN_TYPES:?}"
    );
}

/// The grammar sources the mirror publishes, in the order they are digested.
const PUBLISHED_GRAMMAR: [&str; 4] = [
    "tools/tree-sitter-awl/grammar.js",
    "tools/tree-sitter-awl/src/grammar.json",
    "tools/tree-sitter-awl/src/node-types.json",
    "tools/tree-sitter-awl/src/parser.c",
];

/// Digest of what a mirror commit must be carrying for the pin to be current.
///
/// Each file contributes its path and its length as well as its bytes, so a
/// rename or a shift of content between files cannot leave the digest unchanged.
fn published_grammar_digest() -> String {
    let mut hasher = Sha256::new();
    for relative in PUBLISHED_GRAMMAR {
        let bytes = std::fs::read(repo_root().join(relative))
            .unwrap_or_else(|error| unreachable!("{relative} is not readable: {error}"));
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(&bytes);
    }
    let mut hex = String::new();
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}")
            .unwrap_or_else(|error| unreachable!("writing hex into a String cannot fail: {error}"));
    }
    hex
}

/// Read a `key = "value"` from the manifest's `[grammars.awl]` table, comments
/// included.
///
/// Scoped to that table on purpose: `repository` is also a top-level key naming
/// the aion repository itself, and reading the file as a flat list of lines
/// would silently answer with that one — sending the publish-before-pin check
/// at the wrong remote and passing.
fn manifest_value(manifest: &str, key: &str) -> Option<String> {
    manifest
        .split_once("[grammars.awl]")
        .map_or("", |(_, table)| table)
        .lines()
        // The table ends where the next one begins.
        .take_while(|line| !line.trim_start().starts_with('['))
        .map(str::trim_start)
        .map(|line| line.trim_start_matches('#').trim_start())
        .find_map(|line| {
            let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
            let quoted = rest.trim().strip_prefix('"')?;
            quoted.split('"').next().map(str::to_owned)
        })
}

/// The Zed pin names a mirror commit carrying THIS grammar.
///
/// Zed does not build the grammar from this repository — it fetches
/// `tools/tree-sitter-awl`'s published mirror at a fixed revision. So a grammar
/// change here reaches Zed only after the mirror is pushed and the `rev` bumped,
/// and until 2026-07-31 nothing said so out loud: the pin sat at `fa1afb22`
/// while the grammar moved 436 commits underneath it.
///
/// The digest recorded beside the pin is what makes the staleness detectable
/// offline. Change the grammar and this fires; the only way to quiet it is to
/// do the thing that was missed — push the mirror, then record the new pair.
#[test]
fn the_zed_pin_names_a_mirror_commit_carrying_this_grammar() {
    let manifest = read("editors/zed-awl/extension.toml");
    let Some(recorded) = manifest_value(&manifest, "grammar-sha256") else {
        unreachable!(
            "editors/zed-awl/extension.toml carries no `# grammar-sha256 = \"…\"` line, so \
             nothing relates its `rev` to the grammar it is supposed to publish"
        )
    };
    let Some(rev) = manifest_value(&manifest, "rev") else {
        unreachable!("editors/zed-awl/extension.toml carries no `rev`")
    };
    assert_eq!(
        rev.len(),
        40,
        "the Zed `rev` must be a full 40-character sha, not `{rev}` — an abbreviated or symbolic \
         revision is not a fence"
    );

    let actual = published_grammar_digest();
    assert_eq!(
        actual,
        recorded,
        "\nThe AWL grammar has changed since the Zed pin was recorded, so Zed is still fetching \
         the old parser and colouring the old language.\n\nTo clear this:\n  1. cd \
         tools/tree-sitter-awl && tree-sitter generate && tree-sitter test\n  2. sync \
         grammar.js, src/ and test/corpus/ into the tree-sitter-awl mirror checkout, commit and \
         push it\n  3. verify anonymously: git ls-remote {} refs/heads/main\n  4. set `rev` to \
         that sha and `# grammar-sha256` to {actual}\n\nDo not simply update the digest — that \
         records that the mirror was pushed when it was not.\n",
        manifest_value(&manifest, "repository").unwrap_or_default()
    );
}

/// The recorded pin is actually present on the public mirror.
///
/// This is the network half of the 2026-07-12 publish-before-pin ruling: a `rev`
/// naming a sha that never reached the mirror resolves fine from an
/// authenticated checkout and dies from the anonymous fetch that Zed performs.
/// It cannot run offline, so it is gated at runtime rather than with
/// `#[ignore]`, and it announces itself when it does not run.
#[test]
fn the_recorded_pin_is_present_on_the_public_mirror() {
    if std::env::var("AION_VERIFY_MIRROR_PIN").as_deref() != Ok("1") {
        println!(
            "SKIPPED: the publish-before-pin check reaches the network. Run it with \
             AION_VERIFY_MIRROR_PIN=1 before landing any change to editors/zed-awl/extension.toml."
        );
        return;
    }

    let manifest = read("editors/zed-awl/extension.toml");
    let Some(repository) = manifest_value(&manifest, "repository") else {
        unreachable!("editors/zed-awl/extension.toml carries no `repository`")
    };
    let Some(rev) = manifest_value(&manifest, "rev") else {
        unreachable!("editors/zed-awl/extension.toml carries no `rev`")
    };

    let output = std::process::Command::new("git")
        .args(["ls-remote", &repository, "refs/heads/main"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap_or_else(|error| unreachable!("git ls-remote could not be run: {error}"));
    assert!(
        output.status.success(),
        "anonymous `git ls-remote {repository}` failed, which is exactly what a Zed user's \
         grammar fetch does:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let listing = String::from_utf8_lossy(&output.stdout);
    let head = listing.split_whitespace().next().unwrap_or_default();
    assert_eq!(
        head, rev,
        "the public mirror's main is at {head}, but editors/zed-awl/extension.toml pins {rev}. \
         Either the pinned commit was never pushed — the failure this check exists for — or the \
         mirror has moved ahead and the pin is stale."
    );
}
