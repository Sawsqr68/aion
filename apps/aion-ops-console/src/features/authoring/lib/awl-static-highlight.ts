import { Language, Parser, Query } from 'web-tree-sitter';

// The same vendored artefacts highlight.worker.ts loads: the wasm compiled
// from this repo's tools/tree-sitter-awl and the highlight query beside it.
// The two files in ../assets must always be replaced together, and
// highlight-assets.test.ts gates the compiled wasm against the grammar's own
// keyword list — so this module inherits that guarantee instead of carrying a
// keyword list of its own that could drift.
//
// The worker exists for the editor's keystroke path, where parsing must never
// block the caret. Guide examples are short, static, and parsed once, so they
// are tokenized here on the calling thread with the identical grammar and
// query — one source of truth, two delivery mechanisms.

/** One run of example text, with the capture that colours it (if any). */
export interface AwlToken {
  /** UTF-16 offset of this run in the source — stable identity for rendering. */
  readonly start: number;
  /** The verbatim slice of the source. */
  readonly text: string;
  /** The tree-sitter capture name, or null for uncaptured plain text. */
  readonly capture: string | null;
}

/** A loaded grammar + query pair, ready to tokenize AWL synchronously. */
export interface AwlHighlighter {
  /** Split `source` into contiguous tokens that reassemble to it exactly. */
  readonly tokensOf: (source: string) => readonly AwlToken[];
}

/**
 * What a rendering surface can know about the highlighter: loaded and usable,
 * still loading (render plain, it will arrive), or failed with a reason the
 * surface must show rather than swallow.
 */
export type AwlHighlighterState = AwlHighlighter | 'loading' | { readonly unavailable: string };

let loading: Promise<AwlHighlighter> | undefined;
let ready: AwlHighlighter | null = null;

/**
 * The highlighter if the one-time load has already completed, else null.
 * Synchronous so server-side/static rendering (and the test suite, which
 * awaits {@link loadAwlHighlighter} first) can render highlighted markup
 * without an effect pass.
 */
export function awlHighlighterIfReady(): AwlHighlighter | null {
  return ready;
}

/**
 * Load the vendored grammar and query once, shared by every caller.
 * A failed load clears the cached attempt so a later mount can retry —
 * a fetch that failed mid-session is not a permanent verdict.
 */
export function loadAwlHighlighter(): Promise<AwlHighlighter> {
  if (loading === undefined) {
    loading = initialize().then(
      (highlighter) => {
        ready = highlighter;
        return highlighter;
      },
      (error: unknown) => {
        loading = undefined;
        throw error instanceof Error
          ? error
          : new Error('the AWL highlighter failed to load', { cause: error });
      }
    );
  }
  return loading;
}

/**
 * Fetch one vendored asset, refusing a non-OK response instead of handing a
 * 404 page to the wasm compiler. Exported for its failure-path test.
 */
export async function fetchAsset(url: URL): Promise<Response> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`fetching ${url.href} failed: HTTP ${response.status}`);
  }
  return response;
}

// `new URL(literal, import.meta.url)` is the one asset reference both loaders
// understand: Vite rewrites it to the emitted asset URL at build time, and at
// test time it is a file:// URL bun's fetch reads directly.
async function initialize(): Promise<AwlHighlighter> {
  await Parser.init();
  const [wasmResponse, queryResponse] = await Promise.all([
    fetchAsset(new URL('../assets/awl.wasm', import.meta.url)),
    fetchAsset(new URL('../assets/highlights.scm', import.meta.url)),
  ]);
  const wasmBytes = new Uint8Array(await wasmResponse.arrayBuffer());
  const queryText = await queryResponse.text();
  const language = await Language.load(wasmBytes);
  const parser = new Parser();
  parser.setLanguage(language);
  const query = new Query(language, queryText);
  return { tokensOf: (source) => tokenize(parser, query, source) };
}

/**
 * Run the highlight query over `source` and flatten the captures into
 * contiguous tokens. Where captures overlap, the narrowest node wins each
 * character — the same visual outcome as the editor's nested mark
 * decorations, where the innermost span's colour shows.
 *
 * All indexes are UTF-16 code units (web-tree-sitter's native space), which
 * is also the space `String.prototype.slice` works in — no conversion.
 */
function tokenize(parser: Parser, query: Query, source: string): readonly AwlToken[] {
  const tree = parser.parse(source);
  if (tree === null || tree === undefined) {
    // Unreachable once setLanguage has succeeded; refuse loudly if it ever is.
    throw new Error('the vendored AWL grammar failed to produce a parse tree');
  }
  const owners: (string | null)[] = Array.from({ length: source.length }, () => null);
  const widths: number[] = Array.from({ length: source.length }, () => Number.POSITIVE_INFINITY);
  for (const capture of query.captures(tree.rootNode)) {
    const from = Math.max(capture.node.startIndex, 0);
    const to = Math.min(capture.node.endIndex, source.length);
    const width = capture.node.endIndex - capture.node.startIndex;
    for (let index = from; index < to; index += 1) {
      if (width <= (widths.at(index) ?? Number.POSITIVE_INFINITY)) {
        owners[index] = capture.name;
        widths[index] = width;
      }
    }
  }
  tree.delete();
  return mergeRuns(source, owners);
}

/** Merge per-character capture owners into runs; the runs concatenate back to `source`. */
function mergeRuns(source: string, owners: readonly (string | null)[]): AwlToken[] {
  const tokens: AwlToken[] = [];
  let runStart = 0;
  for (let index = 1; index <= source.length; index += 1) {
    const run = owners.at(runStart) ?? null;
    const next = index < source.length ? (owners.at(index) ?? null) : undefined;
    if (next === undefined || next !== run) {
      tokens.push({ start: runStart, text: source.slice(runStart, index), capture: run });
      runStart = index;
    }
  }
  return tokens;
}
