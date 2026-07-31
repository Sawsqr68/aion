/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

// Every AWL word that should render as syntax, and the ONLY definition of that
// set outside the compiler.
//
// This is a HIGHLIGHTING grammar, not a parsing one: the statement rules below
// match a declaration head and let the rest of the line fall into a generic
// token stream, where `$.keyword` is what gives a word its colour. So this
// array is a colouring decision, not a claim about what the language reserves.
//
// It must cover every variant of `Keyword::ALL` in
// `crates/aion-awl/src/lexer/tokens.rs`, and `grammar_vocabulary_tests.rs` in
// that crate reads this array through `include_str!` and fails when it does
// not. A reserved word missing from here renders in the operator's editor as
// an ordinary name, which is the compiler enforcing something the editor shows
// no sign of.
//
// AWL's POSITIONAL WORDS are deliberately handled unevenly, and the unevenness
// is the correct answer rather than an oversight. `in` is listed; `send`,
// `to`, `query`, `answer` and `run` are NOT. None of the six is reserved — the
// parser accepts each where it expects it and they stay legal as ordinary
// names everywhere else — and this grammar is too coarse to tell the two uses
// apart, so listing one colours it EVERYWHERE.
//
// For `in` that is free: it opens `fork item in items`, and nobody names a
// binding `in`. For the other five it is not. `wait approval timeout 30s ->
// answer` binds a result called `answer`, and the corpus proves it: adding
// those five to this list turns three passing fixtures red, each because an
// ordinary binding suddenly lexes as syntax. Leaving a statement opener plain
// is a cosmetic loss; tinting an author's own binding is the editor lying
// about their code.
const keywords = [
  'workflow', 'input', 'signal', 'outcome', 'type', 'schema', 'worker',
  'action', 'child', 'step', 'subflow', 'after', 'fork', 'join', 'loop',
  'counting', 'until', 'max', 'visits', 'sequential', 'spawn', 'wait',
  'sleep', 'timeout', 'retry', 'every', 'backoff', 'node', 'on', 'failure',
  'when', 'otherwise', 'route', 'success', 'filter', 'map', 'sort', 'count',
  'any', 'all', 'is', 'empty', 'present', 'absent', 'not', 'and', 'or', 'in',
  'const', 'json', 'of', 'distribute', 'sequence', 'collect',
  // Reserved words the grammar had fallen behind on.
  'agent', 'advisory',
];

module.exports = grammar({
  name: 'awl',

  extras: $ => [/[ \t\r]/],

  word: $ => $.identifier,

  rules: {
    source_file: $ => repeat(choice(
      $.workflow_declaration,
      $.input_declaration,
      $.signal_declaration,
      $.outcome_declaration,
      $.type_declaration,
      $.const_declaration,
      $.worker_declaration,
      $.action_declaration,
      $.child_declaration,
      $.subflow_declaration,
      $.step_declaration,
      $.fork_statement,
      $.join_statement,
      $.loop_statement,
      $.distribute_statement,
      $.sequence_statement,
      $.collect_statement,
      $.wait_statement,
      $.sleep_statement,
      $.failure_handler,
      $.route_statement,
      $.spawn_statement,
      $.configuration,
      $.doc_comment,
      $.comment,
      $.json_literal,
      $.schema_literal,
      $.raw_string,
      $.string,
      $.duration,
      $.float,
      $.integer,
      $.boolean,
      $.builtin_type,
      $.type_identifier,
      $.identifier,
      $.pipe_operator,
      $.bind_operator,
      $.comparison_operator,
      $.optional_operator,
      $.punctuation,
      $.keyword,
      $.newline,
    )),

    workflow_declaration: $ => prec(1, seq(alias('workflow', $.keyword), field('name', $.identifier))),
    input_declaration: $ => prec(1, seq(alias('input', $.keyword), field('name', $.identifier))),
    signal_declaration: $ => prec(1, seq(alias('signal', $.keyword), field('name', $.identifier))),
    outcome_declaration: $ => prec(1, seq(alias('outcome', $.keyword), field('name', $.identifier))),
    type_declaration: $ => prec(1, seq(alias('type', $.keyword), field('name', $.type_identifier))),
    const_declaration: $ => prec(1, seq(alias('const', $.keyword), field('name', $.identifier))),
    worker_declaration: $ => prec(1, seq(alias('worker', $.keyword), field('name', $.identifier))),
    action_declaration: $ => prec(1, seq(alias('action', $.keyword), field('name', $.identifier))),
    child_declaration: $ => prec(1, seq(alias('child', $.keyword), field('name', $.identifier))),
    subflow_declaration: $ => prec(1, seq(alias('subflow', $.keyword), field('name', $.identifier))),
    step_declaration: $ => prec(1, seq(alias('step', $.keyword), field('name', $.identifier))),
    fork_statement: $ => prec(1, alias('fork', $.keyword)),
    join_statement: $ => prec(1, alias('join', $.keyword)),
    loop_statement: $ => prec(1, alias('loop', $.keyword)),
    distribute_statement: $ => prec(1, alias('distribute', $.keyword)),
    sequence_statement: $ => prec(1, alias('sequence', $.keyword)),
    collect_statement: $ => prec(1, alias('collect', $.keyword)),
    wait_statement: $ => prec(1, alias('wait', $.keyword)),
    sleep_statement: $ => prec(1, alias('sleep', $.keyword)),
    failure_handler: $ => prec(1, seq(alias('on', $.keyword), alias('failure', $.keyword))),
    route_statement: $ => prec(1, alias('route', $.keyword)),
    spawn_statement: $ => prec(1, alias('spawn', $.keyword)),
    configuration: $ => prec(1, choice(
      alias('node', $.keyword), alias('timeout', $.keyword), alias('retry', $.keyword),
    )),

    // `json { … }` literal: aion-awl's lexer captures the brace-balanced body
    // verbatim as one token (braces inside JSON strings do not count toward
    // balance). Mirrored here as a body node whose strings stay string nodes,
    // so quoting keeps braces raw exactly like the real tokenization.
    json_literal: $ => prec(1, seq(alias('json', $.keyword), field('body', $.json_body))),
    // Inline `schema { … }` type door: same raw brace-balanced capture.
    schema_literal: $ => prec(1, seq(
      alias('schema', $.keyword),
      field('body', alias($.json_body, $.schema_body)),
    )),
    json_body: $ => seq('{', repeat($._json_body_item), '}'),
    _json_body_item: $ => choice(alias($._body_string, $.string), /[^{}"]+/, $._json_nested_braces),
    _json_nested_braces: $ => seq('{', repeat($._json_body_item), '}'),
    // Quoted content inside a braced body: `scan_braced_body` keeps quote
    // state across raw newlines (scanner.rs) — braces stay inert until the
    // closing quote even in a malformed multiline string — unlike top-level
    // `string`, which ends at a newline.
    _body_string: _ => token(/"(?:[^"\\]|\\[\s\S])*"/),

    doc_comment: _ => token(choice(seq('//!', /[^\n]*/), seq('///', /[^\n]*/))),
    comment: _ => token(seq('//', /[^!\/\n][^\n]*|[^\n]?/)),
    // Triple-quoted raw string: verbatim, multi-line, no escapes; the body
    // ends at the FIRST `"""` after the opener (scanner.rs `scan_raw_string`).
    raw_string: _ => token(seq(
      '"""',
      repeat(choice(/[^"]/, seq('"', /[^"]/), seq('""', /[^"]/))),
      '"""',
    )),
    string: _ => token(/"(?:[^"\\\n]|\\.)*"/),
    duration: _ => token(/[0-9]+(?:s|m|h|d)/),
    float: _ => token(/-?(?:[0-9]+\.[0-9]+(?:[eE][+-]?[0-9]+)?|[0-9]+[eE][+-]?[0-9]+)/),
    integer: _ => token(/-?[0-9]+/),
    boolean: _ => choice('true', 'false'),
    builtin_type: _ => choice('Bool', 'Int', 'Float', 'String', 'Nil', 'Dir'),
    type_identifier: _ => token(/[A-Z][A-Za-z0-9_]*/),
    identifier: _ => token(/[a-z_][a-zA-Z0-9_]*/),

    pipe_operator: _ => '|>',
    bind_operator: _ => '->',
    comparison_operator: _ => choice('==', '!=', '<=', '>=', '<', '>', '+', '..'),
    optional_operator: _ => '?',
    punctuation: _ => choice('(', ')', '[', ']', '{', '}', ':', ',', '.', '=', '|'),
    keyword: _ => choice(...keywords),
    newline: _ => /\n/,
  },
});
