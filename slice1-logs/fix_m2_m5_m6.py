"""m2 (honest + pinned drift claim), m5 (one truth about the empty declaration),
m6 (the CLAUDECODE absence assert gets its presence twin)."""

import pathlib

# ------------------------------------------------------------------ m2
p = pathlib.Path("crates/aion-awl/src/printer/harness.rs")
s = p.read_text()
s = s.replace(
    """//! The section prints in a FIXED field order — [`HARNESS_FIELDS`] — rather than the
//! order it was written in, for the same reason an action's config line does: `aion awl
//! fmt` gives one shape per document, so two operators who declared the same launch in
//! different orders end up with the same bytes and their diff shows what actually
//! changed.""",
    """//! The section prints in a FIXED field order — [`HARNESS_FIELDS`]' — rather than the
//! order it was written in, for the same reason an action's config line does: `aion awl
//! fmt` gives one shape per document, so two operators who declared the same launch in
//! different orders end up with the same bytes and their diff shows what actually
//! changed.
//!
//! The printer emits one `if let` per setting rather than WALKING that array: the twelve
//! values have twelve different types, and a table of extractors would trade a readable
//! sequence for a pile of closures. The correspondence is therefore not enforced by
//! construction — it is enforced by [`tests`], which prints a section carrying every
//! field in a scrambled order and asserts the emitted field words are exactly
//! [`HARNESS_FIELDS`], in its order, with nothing else. A field added to the grammar
//! with no print position fails there instead of vanishing on `aion awl fmt`.
//!
//! [`HARNESS_FIELDS`]: crate::ast::HARNESS_FIELDS""",
)
s = s.replace(
    """fn text_list(values: &[HarnessText]) -> String {""",
    """fn text_list(values: &[HarnessText]) -> String {""",
)
s += "\n#[cfg(test)]\nmod tests;\n"
p.write_text(s)
print("m2 printer:", "enforced by [`tests`]" in s)

p = pathlib.Path("crates/aion-awl/src/ast/harness.rs")
s = p.read_text()
s = s.replace(
    """/// Every field word the section admits, in canonical print order.
///
/// The parser refuses anything else by name against this list, and the printer walks
/// it, so the two cannot drift: a field added to the grammar without a print position
/// would print nothing and vanish on `aion awl fmt`.""",
    """/// Every field word the section admits, in canonical print order.
///
/// Two duties, one table. The parser refuses an unknown field by naming this list, and
/// the printer emits the section in exactly this ORDER — enforced by
/// `printer::harness::tests`, because the printer cannot walk an array of twelve
/// differently-typed values without becoming a pile of closures. A field added to the
/// grammar with no print position fails that pin rather than printing nothing and
/// vanishing from a deployed document on `aion awl fmt`.""",
)
p.write_text(s)
print("m2 ast:", "Two duties, one table" in s)

# ------------------------------------------------------------------ m5
p = pathlib.Path("crates/aion-integrations/src/environment.rs")
s = p.read_text()
s = s.replace(
    """    /// The declaration naming NO variable.
    ///
    /// Infallible, because there is no entry to be malformed. It is a real, expressible
    /// state and not a degenerate one: a child launched under it gets an empty
    /// environment, which is legitimate for an absolute command that needs nothing.""",
    """    /// The declaration naming NO variable.
    ///
    /// Infallible, because there is no entry to be malformed. A child launched under it
    /// gets a genuinely empty environment, which is a coherent thing for an absolute
    /// command that needs nothing at all.
    ///
    /// It is reachable only from a hand-written composition root, and that is deliberate
    /// rather than an oversight in the grammar: a worker DOCUMENT cannot express it,
    /// because `env_pass` takes one or more names and there is no spelling for none. An
    /// empty list in a file is indistinguishable from an unfinished line, and the cost of
    /// reading one as the other is an agent launched with no `PATH`, no `HOME` and no
    /// credentials, failing three layers down for a reason nobody wrote. A caller
    /// constructing this in code has stated it unambiguously; an author typing it into a
    /// document has not. If the hermetic-empty case is ever wanted from a document it
    /// needs its own explicit spelling, not a permissive reading of an empty list.""",
)
p.write_text(s)
print("m5:", "needs its own explicit spelling" in s)

# ------------------------------------------------------------------ m6
p = pathlib.Path("crates/aion-cli/src/harness/tests.rs")
s = p.read_text()
s = s.replace(
    """    let mut launch = declared_for(kind);
    launch.env_pass = vec![String::from("PATH")];
    let Some(resolved) = super::settings::resolve(&launch, &document())? else {
        return Err("a build carrying an adapter must resolve the declared kind".into());
    };
    let environment = environment_of(&resolved.config);
    assert_eq!(
        environment.declared(),
        ["PATH"],
        "the declaration is exactly what the document wrote"
    );
    assert!(
        !environment.carries("CLAUDECODE"),
        "a variable the document does not name is absent whatever this process carries"
    );
    Ok(())""",
    """    let mut launch = declared_for(kind);
    launch.env_pass = vec![String::from("PATH")];
    let Some(resolved) = super::settings::resolve(&launch, &document())? else {
        return Err("a build carrying an adapter must resolve the declared kind".into());
    };
    let environment = environment_of(&resolved.config);

    // The PRESENCE twin. This resolution reads THIS process's environment, so the
    // absence assertion below is only worth something if the process really carries
    // something that did not cross — otherwise it would pass just as well against an
    // implementation that resolved nothing at all. Cargo sets `CARGO_PKG_NAME` on every
    // test process, and it is not declared.
    assert!(
        std::env::var_os("CARGO_PKG_NAME").is_some(),
        "this test needs a variable the launching process really carries, or the \\
         absence assertion below proves nothing"
    );
    assert!(
        environment.carries("PATH"),
        "the declared variable IS carried, so an absent reading cannot come from an \\
         environment that resolved nothing: {:?}",
        environment.pairs()
    );
    assert_eq!(
        environment.declared(),
        ["PATH"],
        "the declaration is exactly what the document wrote"
    );
    assert!(
        !environment.carries("CARGO_PKG_NAME"),
        "a variable this process really carries, and the document does not name, does \\
         not cross: {:?}",
        environment.pairs()
    );
    assert!(
        !environment.carries("CLAUDECODE"),
        "a variable the document does not name is absent whatever this process carries"
    );
    Ok(())""",
)
p.write_text(s)
print("m6:", "The PRESENCE twin" in s)
