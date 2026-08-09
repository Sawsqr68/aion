import { beforeAll, describe, expect, test } from 'bun:test';

import { awlGuide } from './awl-guide';
import {
  type AwlHighlighter,
  awlHighlighterIfReady,
  fetchAsset,
  loadAwlHighlighter,
} from './awl-static-highlight';

// The static tokenizer feeds the guide's example rendering, so its contract
// is exercised against the guide's own corpus: every checked example the
// console ships, not a hand-picked fixture whose right and plausible-wrong
// answers could coincide.

let highlighter: AwlHighlighter;

beforeAll(async () => {
  highlighter = await loadAwlHighlighter();
});

describe('the static AWL highlighter', () => {
  test('is a shared singleton, synchronously visible once loaded', async () => {
    expect(awlHighlighterIfReady()).toBe(highlighter);
    expect(await loadAwlHighlighter()).toBe(highlighter);
  });

  /**
   * The invariant, over the whole corpus: tokens are contiguous, start where
   * they say they start, and concatenate back to the source byte-for-byte.
   * A tokenizer that drops or duplicates a character would corrupt the very
   * example the author is about to paste into a file.
   */
  test('tokens reassemble every guide example exactly', () => {
    expect(awlGuide.entries.length).toBeGreaterThan(50); // non-vacuity: the corpus loaded
    for (const entry of awlGuide.entries) {
      for (const example of entry.examples) {
        const tokens = highlighter.tokensOf(example);
        let offset = 0;
        for (const token of tokens) {
          expect(token.start).toBe(offset);
          expect(token.text.length).toBeGreaterThan(0);
          offset += token.text.length;
        }
        expect(tokens.map((token) => token.text).join('')).toBe(example);
      }
    }
  });

  test('colours a keyword and leaves an author’s own binding plain', () => {
    const tokens = highlighter.tokensOf('wait approval timeout 30s -> answer\n');
    const captureOf = (text: string) =>
      tokens.filter((token) => token.text === text).map((token) => token.capture);
    expect(captureOf('wait')).toEqual(['keyword']);
    expect(captureOf('30s')).toEqual(['number']);
    expect(captureOf('->')).toEqual(['operator']);
    // `answer` is a binding this grammar deliberately leaves uncoloured — the
    // same fact the vendored-artefact gate pins from the grammar's side.
    const answer = tokens.find((token) => token.text.includes('answer'));
    expect(answer?.capture).toBeNull();
  });

  test('an empty document tokenizes to nothing rather than failing', () => {
    expect(highlighter.tokensOf('')).toEqual([]);
  });
});

describe('fetchAsset', () => {
  test('refuses a non-OK response with the URL and status named', async () => {
    const original = globalThis.fetch;
    globalThis.fetch = Object.assign(async () => new Response('gone', { status: 404 }), {
      preconnect: original.preconnect,
    });
    try {
      await expect(fetchAsset(new URL('https://console.invalid/awl.wasm'))).rejects.toThrow(
        'fetching https://console.invalid/awl.wasm failed: HTTP 404'
      );
    } finally {
      globalThis.fetch = original;
    }
  });

  test('a missing asset fails loudly by the time its body is read', async () => {
    // bun's file:// fetch resolves the Response and surfaces ENOENT at the
    // body read — the read the loader always performs — while a browser 404
    // is caught by the ok check above. Either way the load fails, never
    // hands empty bytes onward.
    await expect(
      fetchAsset(new URL('file:///this-asset-does-not-exist/awl.wasm')).then((response) =>
        response.arrayBuffer()
      )
    ).rejects.toBeDefined();
  });
});
