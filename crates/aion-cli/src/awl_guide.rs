//! `aion awl guide` — the language's glossary, at the command line.
//!
//! Every sentence printed here comes from `aion_awl::guide`, the same table
//! the language server hovers and the ops console renders. Nothing in this
//! module writes prose about AWL, and nothing in it may: a `--help` string
//! that explained `distribute` in its own words would be a third account of
//! the language, and the day the language moved it would still read
//! plausibly.
//!
//! The `--json` form is the console's input. It is checked into the console's
//! assets rather than fetched at runtime — the console must teach the
//! language with no server reachable — and
//! `tests/console_guide_asset.rs` refuses any drift between the two.

use std::collections::BTreeMap;
use std::process::ExitCode;

use aion_awl::guide::{self, GrammarForm, GrammarPosition, GuideEntry, WordKind};
use anyhow::Context as _;
use serde::Serialize;

/// One glossary entry, as the console reads it.
///
/// `citation` is carried rather than recomputed so that the console prints
/// the byte-identical pointer the CLI prints. Two renderings of one fact are
/// two facts as soon as one of them is edited. `examples` is the entry's
/// complete checked documents, verbatim, one per honest form of the word.
/// `form` and `positions` are the grammar axis, derived from the same table
/// the language server's completion offers words from — the console groups
/// by `form` and looks both up in the document-level `forms` and `positions`
/// inventories.
#[derive(Debug, Serialize)]
struct WireEntry {
    word: &'static str,
    kind: &'static str,
    summary: &'static str,
    examples: &'static [&'static str],
    section: WireSection,
    citation: String,
    form: &'static str,
    positions: Vec<&'static str>,
}

/// The reference section an entry cites.
#[derive(Debug, Serialize)]
struct WireSection {
    number: u8,
    title: &'static str,
}

/// One grammatical form, as the console's grouping axis reads it.
#[derive(Debug, Serialize)]
struct WireForm {
    id: &'static str,
    label: &'static str,
    description: &'static str,
}

/// One grammar position a word may stand in, with its rendered label.
#[derive(Debug, Serialize)]
struct WirePosition {
    id: &'static str,
    label: &'static str,
}

/// The whole glossary: the paths every surface points authors at, the
/// grammar taxonomy in display order, the full reference text, and the
/// entries themselves.
#[derive(Debug, Serialize)]
struct WireGuide {
    guide: &'static str,
    reference: &'static str,
    authoring: &'static str,
    workers: &'static str,
    commands: &'static str,
    declared_commands: &'static str,
    forms: Vec<WireForm>,
    positions: Vec<WirePosition>,
    entries: Vec<WireEntry>,
    reference_text: &'static str,
}

impl WireEntry {
    fn of(entry: GuideEntry) -> anyhow::Result<Self> {
        let form = guide::form_of(entry.word()).with_context(|| {
            format!(
                "`{}` has a glossary entry but the grammar table offers it in no position, so it \
                 has no form; place it in `aion_awl::guide::grammar`",
                entry.word()
            )
        })?;
        Ok(Self {
            word: entry.word(),
            kind: entry.kind().as_str(),
            summary: entry.summary(),
            examples: entry.examples(),
            section: WireSection {
                number: entry.section().number(),
                title: entry.section().title(),
            },
            citation: entry.citation(),
            form: form.id(),
            positions: guide::positions_of(entry.word())
                .into_iter()
                .map(GrammarPosition::id)
                .collect(),
        })
    }
}

/// The glossary as canonical JSON, ending in a newline.
///
/// Field order is the declaration order of the structs above and entry order
/// is the language's own inventory order, so the same table always serialises
/// to the same bytes — which is what lets the console asset be gated by
/// comparison rather than by inspection.
pub(crate) fn json(word: Option<&str>) -> anyhow::Result<String> {
    let entries = match word {
        Some(word) => guide::for_word(word)
            .into_iter()
            .map(WireEntry::of)
            .collect::<anyhow::Result<Vec<_>>>()?,
        None => guide::all()
            .into_iter()
            .map(WireEntry::of)
            .collect::<anyhow::Result<Vec<_>>>()?,
    };
    let document = WireGuide {
        guide: guide::GUIDE_PATH,
        reference: guide::REFERENCE_PATH,
        authoring: guide::AUTHORING_PATH,
        workers: guide::WORKERS_PATH,
        commands: guide::COMMANDS_PATH,
        declared_commands: guide::DECLARED_COMMANDS_PATH,
        forms: GrammarForm::ALL
            .into_iter()
            .map(|form| WireForm {
                id: form.id(),
                label: form.label(),
                description: form.description(),
            })
            .collect(),
        positions: GrammarPosition::ALL
            .into_iter()
            .map(|position| WirePosition {
                id: position.id(),
                label: position.label(),
            })
            .collect(),
        entries,
        reference_text: guide::reference_text(),
    };
    let mut rendered =
        serde_json::to_string_pretty(&document).context("the glossary could not serialise")?;
    rendered.push('\n');
    Ok(rendered)
}

/// Runs `aion awl guide`.
pub(crate) fn run(word: Option<&str>, json_output: bool, reference: bool) -> ExitCode {
    if reference {
        // The full reference, embedded in this binary at build time — the
        // same file the repository's guide_reference_gate reads, so what an
        // installed binary prints is what the gate verified.
        print!("{}", guide::reference_text());
        return ExitCode::SUCCESS;
    }
    if let Some(word) = word {
        let Some(entry) = guide::for_word(word) else {
            eprintln!(
                "error: `{word}` is not a word of AWL; run `aion awl guide` for every word the \
                 language has"
            );
            return ExitCode::FAILURE;
        };
        if json_output {
            return emit_json(Some(entry.word()));
        }
        print!("{}", render_one(entry));
        return ExitCode::SUCCESS;
    }
    if json_output {
        return emit_json(None);
    }
    print!("{}", render_all());
    ExitCode::SUCCESS
}

fn emit_json(word: Option<&str>) -> ExitCode {
    match json(word) {
        Ok(rendered) => {
            print!("{rendered}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: the glossary could not be rendered as JSON: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn kind_noun(kind: WordKind) -> &'static str {
    match kind {
        WordKind::Reserved => "reserved word",
        WordKind::Positional => "positional word",
    }
}

/// One complete example document, indented four spaces as a display block.
fn indented(example: &str) -> String {
    let mut block = String::new();
    for line in example.lines() {
        if line.is_empty() {
            block.push('\n');
        } else {
            block.push_str("    ");
            block.push_str(line);
            block.push('\n');
        }
    }
    block
}

/// One word: its meaning, its complete checked examples indented as blocks —
/// numbered when the word has more than one form — then the citation and how
/// to open the full reference from this binary.
fn render_one(entry: GuideEntry) -> String {
    let numbered = entry.examples().len() > 1;
    let blocks = entry
        .examples()
        .iter()
        .enumerate()
        .map(|(index, example)| {
            if numbered {
                format!("  Example {}:\n\n{}", index + 1, indented(example))
            } else {
                indented(example)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}  ({})\n  {}\n\n{blocks}\n  {}\n  Full reference: {}\n",
        entry.word(),
        kind_noun(entry.kind()),
        entry.summary(),
        entry.citation(),
        guide::REFERENCE_COMMAND
    )
}

/// The whole glossary, grouped by the reference section each word belongs to.
///
/// The column width is measured from the words themselves rather than fixed,
/// so a longer word added to the language cannot silently push the summaries
/// out of alignment.
fn render_all() -> String {
    let entries = guide::all();
    let width = entries
        .iter()
        .map(|entry| entry.word().len())
        .max()
        .unwrap_or_default();

    let mut lines: Vec<String> = vec![
        "AWL — every word of the language, and where it is explained in full.".to_owned(),
        "Run `aion awl guide <word>` for one word with a checked example.".to_owned(),
        String::new(),
        format!("  Full reference   {}", guide::REFERENCE_COMMAND),
        format!("  Guide            {}", guide::GUIDE_PATH),
        format!("  Reference        {}", guide::REFERENCE_PATH),
        format!("  Authoring path   {}", guide::AUTHORING_PATH),
        format!("  Workers          {}", guide::WORKERS_PATH),
        format!("  Commands         {}", guide::COMMANDS_PATH),
        format!("  Declared bodies  {}", guide::DECLARED_COMMANDS_PATH),
    ];

    let mut sections: BTreeMap<u8, Vec<GuideEntry>> = BTreeMap::new();
    for entry in entries {
        sections
            .entry(entry.section().number())
            .or_default()
            .push(entry);
    }
    for (number, grouped) in sections {
        let title = grouped.first().map_or("", |entry| entry.section().title());
        lines.push(String::new());
        lines.push(format!("§{number} — {title}"));
        for entry in grouped {
            lines.push(format!(
                "  {:width$}  {}",
                entry.word(),
                entry.summary(),
                width = width
            ));
        }
    }
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

#[cfg(test)]
#[path = "awl_guide_tests.rs"]
mod awl_guide_tests;
