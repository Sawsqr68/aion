import {
  history,
  historyKeymap,
  indentLess,
  indentMore,
  indentWithTab,
} from '@codemirror/commands';
import { indentUnit } from '@codemirror/language';
import {
  EditorState,
  RangeSet,
  type StateCommand,
  StateEffect,
  StateField,
} from '@codemirror/state';
import {
  Decoration,
  type DecorationSet,
  EditorView,
  GutterMarker,
  gutter,
  hoverTooltip,
  keymap,
  showTooltip,
  type Tooltip,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view';

import { syntaxClassFor } from '../lib/highlight-classes';
import { diagnosticTooltipDom } from './diagnostic-tooltip-dom';
import type {
  AuthoringEditor,
  CursorChange,
  EditorChange,
  EditorDecorations,
  EditorOptions,
  HighlightSpan,
  LayoutMetrics,
  MouseHover,
  TextDelta,
} from './types';

const setHighlights = StateEffect.define<DecorationSet>();
const setHostDecorations = StateEffect.define<DecorationSet>();
const setGutters = StateEffect.define<RangeSet<GutterMarker>>();
const setDiagnosticSpans = StateEffect.define<readonly DiagnosticSpan[]>();
const remoteOrigin = StateEffect.define<boolean>();

/** One underline's extent and message, kept for the tooltip surfaces. */
type DiagnosticSpan = { from: number; to: number; message: string };

const highlightsField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update: (value, transaction) => {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) if (effect.is(setHighlights)) next = effect.value;
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});
const hostDecorationsField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update: (value, transaction) => {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects)
      if (effect.is(setHostDecorations)) next = effect.value;
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});
const guttersField = StateField.define<RangeSet<GutterMarker>>({
  create: () => RangeSet.empty,
  update: (value, transaction) => {
    for (const effect of transaction.effects) if (effect.is(setGutters)) return effect.value;
    return value.map(transaction.changes);
  },
});

/**
 * The diagnostic spans behind the underline decorations, kept as plain data
 * so the hover and cursor tooltips can answer "what is wrong HERE" — the
 * decorations themselves are write-only paint. Positions map through edits
 * exactly as the underlines do, and each `setDecorations` call replaces the
 * set whole.
 */
const diagnosticSpansField = StateField.define<readonly DiagnosticSpan[]>({
  create: () => [],
  update: (value, transaction) => {
    let next = transaction.docChanged
      ? value.map((span) => ({
          ...span,
          from: transaction.changes.mapPos(span.from),
          to: transaction.changes.mapPos(span.to),
        }))
      : value;
    for (const effect of transaction.effects)
      if (effect.is(setDiagnosticSpans)) next = effect.value;
    return next;
  },
});

/** The diagnostic spans covering `position`, in document order. */
function diagnosticsAt(
  spans: readonly DiagnosticSpan[],
  position: number
): readonly DiagnosticSpan[] {
  return spans.filter((span) => position >= span.from && position <= span.to);
}

/**
 * The keyboard half of diagnostic tooltips: when the CURSOR rests inside a
 * diagnostic range, the same tooltip the pointer would get is shown at the
 * cursor (CodeMirror's cursor-tooltip convention) — remedy prose must not
 * require a mouse. Pointer-driven selections are left to the hover tooltip,
 * and a document edit hides the tooltip until fresh diagnostics arrive
 * rather than pinning stale prose beside live typing.
 */
const cursorDiagnosticField = StateField.define<Tooltip | null>({
  create: () => null,
  update: (value, transaction) => {
    if (transaction.docChanged) return null;
    const refreshed = transaction.effects.some((effect) => effect.is(setDiagnosticSpans));
    if (transaction.selection === undefined && !refreshed) return value;
    if (transaction.selection !== undefined && transaction.isUserEvent('select.pointer'))
      return null;
    return cursorDiagnosticTooltip(transaction.state);
  },
  provide: (field) => showTooltip.from(field),
});

function cursorDiagnosticTooltip(state: EditorState): Tooltip | null {
  const selection = state.selection.main;
  if (!selection.empty) return null;
  const found = diagnosticsAt(state.field(diagnosticSpansField), selection.head);
  if (found.length === 0) return null;
  return {
    pos: selection.head,
    above: true,
    create: () => ({ dom: diagnosticTooltipDom(found.map((span) => span.message)) }),
  };
}

/**
 * The pointer half: a real CM6 hover tooltip over the underlined range,
 * replacing the native `title` attribute this seam used to set (≈1s delay,
 * no wrapping, no styling, no keyboard path). Yields to the cursor tooltip
 * when both would say the same thing at the same spot.
 */
const diagnosticHover = hoverTooltip((view, position) => {
  const found = diagnosticsAt(view.state.field(diagnosticSpansField), position);
  if (found.length === 0) return null;
  const cursor = view.state.field(cursorDiagnosticField);
  if (cursor !== null && diagnosticsAt(found, cursor.pos).length > 0) return null;
  return {
    pos: Math.min(...found.map((span) => span.from)),
    end: Math.max(...found.map((span) => span.to)),
    above: true,
    create: () => ({ dom: diagnosticTooltipDom(found.map((span) => span.message)) }),
  };
});

/**
 * Answer the diagnostic messages a tooltip would show at `position`, over a
 * headless state built from the REAL production extension list — the same
 * array `createCodeMirrorEditor` installs, so the pin covers the wiring, not
 * a hand-copied subset. Exported for the seam tests (tooltip targeting,
 * including position mapping through an edit), in the
 * `applyTabBindingForTest` shape.
 */
export function diagnosticMessagesForTest(
  content: string,
  spans: readonly DiagnosticSpan[],
  position: number,
  edit?: { from: number; to: number; insert: string }
): string[] {
  let state = EditorState.create({ doc: content, extensions: editorExtensions({}) });
  state = state.update({ effects: setDiagnosticSpans.of(spans) }).state;
  if (edit !== undefined) {
    state = state.update({ changes: edit }).state;
  }
  return diagnosticsAt(state.field(diagnosticSpansField), position).map((span) => span.message);
}

/** A follow-up transaction the tooltip probe applies after installing spans. */
type TooltipProbeStep =
  | { kind: 'pointer-select'; to: number }
  | { kind: 'keyboard-select'; to: number }
  | { kind: 'edit'; from: number; to: number; insert: string };

/**
 * Probe BOTH tooltip halves out of the production extension list, headless.
 *
 * Builds an `EditorState` from `editorExtensions` with the cursor at
 * `cursor`, installs `spans`, optionally applies one follow-up transaction
 * (`then`) — a pointer selection, a keyboard selection, or a document edit,
 * the three inputs the cursor tooltip's interplay rules discriminate — and
 * answers what both halves hold in the final state.
 *
 * `hoverActive` is the hover extension's active-tooltip count read out of
 * the same state: the value is asserted by the seam tests, so the read is
 * load-bearing — if `diagnosticHover` falls out of the production array,
 * `state.field` throws and the suite goes red.
 */
export function cursorDiagnosticForTest(
  content: string,
  spans: readonly DiagnosticSpan[],
  cursor: number,
  then?: TooltipProbeStep
): { cursor: { pos: number } | null; hoverActive: number } {
  let state = EditorState.create({
    doc: content,
    selection: { anchor: cursor },
    extensions: editorExtensions({}),
  });
  state = state.update({ effects: setDiagnosticSpans.of(spans) }).state;
  if (then !== undefined) {
    state = state.update(
      then.kind === 'edit'
        ? { changes: { from: then.from, to: then.to, insert: then.insert } }
        : {
            selection: { anchor: then.to },
            userEvent: then.kind === 'pointer-select' ? 'select.pointer' : 'select',
          }
    ).state;
  }
  const hoverActive = state.field(diagnosticHover.active).length;
  const tooltip = state.field(cursorDiagnosticField);
  return { cursor: tooltip === null ? null : { pos: tooltip.pos }, hoverActive };
}

/**
 * AWL indentation is two SPACES per level — the lexer rejects tab characters
 * in indentation outright ("tabs are not allowed in indentation"), so the
 * indent unit must never be a tab. `indentWithTab` binds Tab/Shift-Tab to
 * indent-more/indent-less, which insert/remove this unit. The accessibility
 * escape hatch CodeMirror documents is untouched: it lives in the view's
 * keydown path (Escape arms tab-focus mode for two seconds, during which Tab
 * bypasses keymaps and moves focus), ahead of this binding.
 */
const AWL_INDENT_UNIT = '  ';
const indentation = [indentUnit.of(AWL_INDENT_UNIT), keymap.of([indentWithTab])];

/**
 * Run one side of the Tab binding over a headless document, exported so the
 * seam tests can pin the indentation contract without a DOM. `indentWithTab`
 * binds Tab to `indentMore` and Shift-Tab to `indentLess`; these are the
 * same command objects the editor installs, run against the same indent-unit
 * facet value.
 */
export function applyTabBindingForTest(
  content: string,
  selection: { anchor: number; head?: number },
  key: 'Tab' | 'Shift-Tab'
): string {
  let state = EditorState.create({
    doc: content,
    selection,
    extensions: [indentUnit.of(AWL_INDENT_UNIT)],
  });
  const command: StateCommand = key === 'Tab' ? indentMore : indentLess;
  command({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  return state.doc.toString();
}

/**
 * Attempt a document change against a state built with (or without) the
 * read-only extensions, exported so the seam tests can pin the refusal over a
 * headless document — the same shape as `applyTabBindingForTest`.
 *
 * Returns the document after the attempt. A read-only state must return it
 * unchanged; the editable state is the control that proves the attempt is a
 * real one.
 */
export function applyInsertForTest(content: string, insert: string, readOnly: boolean): string {
  const state = EditorState.create({
    doc: content,
    extensions: readOnly ? readOnlyDocument : [],
  });
  return state.update({ changes: { from: 0, to: 0, insert } }).state.doc.toString();
}

class TextMarker extends GutterMarker {
  constructor(
    readonly text: string,
    readonly markerClass: string
  ) {
    super();
  }
  override toDOM() {
    const element = document.createElement('span');
    element.className = this.markerClass;
    element.textContent = this.text;
    return element;
  }
}

/**
 * A read-only document: no keystroke, no paste, no drag-drop, and no
 * programmatic dispatch can change it.
 *
 * Three extensions, because each closes a different door and none of them
 * closes all three:
 *
 * - `EditorState.changeFilter` returning false SUPPRESSES the document changes
 *   of every transaction. This is the one that holds against code: `readOnly`
 *   alone does NOT stop a dispatched `state.update({ changes })`, which was
 *   measured, not assumed (`read-only.test.ts`).
 * - `EditorState.readOnly` stops the editing commands and the view's own input
 *   handling from even trying.
 * - `EditorView.editable` takes the contenteditable off the DOM node, so the
 *   browser offers no editing caret and assistive technology does not announce
 *   the region as editable.
 *
 * Decorations, highlights, gutters, and cursor moves are transaction EFFECTS
 * and selections, not document changes, so they keep working.
 */
const readOnlyDocument = [
  EditorState.changeFilter.of(() => false),
  EditorState.readOnly.of(true),
  EditorView.editable.of(false),
];

/** The editor's visual theme, semantic tokens only. */
function editorTheme() {
  return EditorView.theme({
    '&': {
      height: '100%',
      backgroundColor: 'var(--surface-elevated)',
      color: 'var(--foreground)',
      fontSize: '0.8125rem',
    },
    '.cm-scroller': { lineHeight: '1.65' },
    '.cm-content': {
      padding: '0.75rem 0',
      fontFamily: 'var(--font-mono)',
      caretColor: 'var(--accent-primary)',
    },
    '.cm-line': { padding: '0 1rem' },
    '.cm-cursor': { borderLeftColor: 'var(--foreground)' },
    '.cm-gutters': {
      backgroundColor: 'var(--surface-base)',
      color: 'var(--muted-foreground)',
      borderRightColor: 'var(--border)',
    },
    '.cm-gutterElement': { padding: '0 0.625rem' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--accent)' },
    '&.cm-focused .cm-selectionBackground, ::selection': {
      backgroundColor: 'var(--secondary)',
    },
    '.cm-tooltip': {
      backgroundColor: 'var(--surface-base)',
      color: 'var(--foreground)',
      border: '1px solid var(--border)',
      borderRadius: '0.5rem',
      boxShadow: 'var(--shadow-md)',
    },
    '.awl-diagnostic-tooltip': {
      maxWidth: '28rem',
      padding: '0.5rem 0.625rem',
      fontFamily: 'var(--font-sans)',
      fontSize: '0.75rem',
      lineHeight: '1.5',
      whiteSpace: 'normal',
    },
    '.awl-diagnostic-tooltip-entry + .awl-diagnostic-tooltip-entry': {
      marginTop: '0.5rem',
      paddingTop: '0.5rem',
      borderTop: '1px solid var(--border)',
    },
    '.awl-diagnostic-remedy': {
      marginTop: '0.25rem',
      color: 'var(--muted-foreground)',
    },
    '.awl-diagnostic-tooltip code': {
      fontFamily: 'var(--font-mono)',
      fontSize: '0.6875rem',
      padding: '0 0.25rem',
      borderRadius: '0.25rem',
      backgroundColor: 'var(--accent)',
    },
  });
}

/**
 * The one production extension list, shared verbatim by the live editor and
 * the headless seam tests. A single source on purpose: a test that builds
 * its state from a hand-copied subset cannot notice an extension falling out
 * of the real editor, so the tooltip-wiring pins query THIS array.
 *
 * Only the per-instance listener wiring (update listener, DOM event
 * handlers, scroll plugin — all closures over one editor's listener sets)
 * is appended separately in `createCodeMirrorEditor`.
 */
function editorExtensions(options: EditorOptions) {
  return [
    ...(options.readOnly === true ? readOnlyDocument : []),
    history(),
    keymap.of(historyKeymap),
    indentation,
    highlightsField,
    hostDecorationsField,
    // Order is load-bearing: `cursorDiagnosticField` reads the span field
    // from the new state during its update, so the span field must come
    // first in the configuration.
    diagnosticSpansField,
    cursorDiagnosticField,
    diagnosticHover,
    guttersField,
    gutter({
      class: 'authoring-diagnostic-gutter',
      markers: (editor) => editor.state.field(guttersField),
    }),
    EditorView.lineWrapping,
    editorTheme(),
  ];
}

/** The only CodeMirror-aware implementation of the editor seam. */
export function createCodeMirrorEditor(
  parent: HTMLElement,
  initialContent: string,
  options: EditorOptions = {}
): AuthoringEditor {
  const changeListeners = new Set<(change: EditorChange) => void>();
  const cursorListeners = new Set<(change: CursorChange) => void>();
  const scrollListeners = new Set<(metrics: LayoutMetrics) => void>();
  const hoverListeners = new Set<(hover: MouseHover | null) => void>();

  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: initialContent,
      extensions: [
        ...editorExtensions(options),
        EditorView.updateListener.of((update) =>
          notifyEditorUpdate(update, changeListeners, cursorListeners)
        ),
        EditorView.domEventHandlers({
          mousemove(event, editor) {
            const position = editor.posAtCoords({ x: event.clientX, y: event.clientY });
            const hover: MouseHover | null =
              position === null
                ? null
                : { position, clientX: event.clientX, clientY: event.clientY };
            for (const listener of hoverListeners) listener(hover);
          },
          mouseleave() {
            for (const listener of hoverListeners) listener(null);
          },
        }),
        ViewPlugin.fromClass(
          class {
            constructor(readonly editor: EditorView) {}
            update(update: ViewUpdate) {
              if (update.viewportChanged || update.geometryChanged) {
                const metrics = readMetrics(update.view);
                for (const listener of scrollListeners) listener(metrics);
              }
            }
          }
        ),
      ],
    }),
  });

  return {
    getContent: () => view.state.doc.toString(),
    applyTextDelta(ranges: readonly TextDelta[], origin = 'local') {
      if (ranges.length === 0) return;
      const scrollTop = view.scrollDOM.scrollTop;
      const scrollLeft = view.scrollDOM.scrollLeft;
      const effects = origin === 'remote' ? [remoteOrigin.of(true)] : [];
      view.dispatch({
        changes: ranges.map(({ from, to, insert }) => ({ from, to, insert })),
        effects,
        scrollIntoView: false,
      });
      view.scrollDOM.scrollTo({ top: scrollTop, left: scrollLeft });
    },
    onChange(listener) {
      changeListeners.add(listener);
      return () => changeListeners.delete(listener);
    },
    positionToPixel(position) {
      const coordinates = view.coordsAtPos(position);
      const root = view.dom.getBoundingClientRect();
      return coordinates === null
        ? null
        : {
            left: coordinates.left - root.left,
            top: coordinates.top - root.top,
            bottom: coordinates.bottom - root.top,
          };
    },
    getLayoutMetrics: () => readMetrics(view),
    onScroll(listener) {
      scrollListeners.add(listener);
      return () => scrollListeners.delete(listener);
    },
    setHighlightSpans(spans) {
      view.dispatch({ effects: setHighlights.of(highlightDecorations(view, spans)) });
    },
    setDecorations(decorations) {
      view.dispatch({
        effects: [
          setHostDecorations.of(hostDecorationSet(view, decorations)),
          setGutters.of(gutterMarkers(decorations, view)),
          setDiagnosticSpans.of(diagnosticSpans(decorations, view.state.doc.length)),
        ],
      });
    },
    onMouseHover(listener) {
      hoverListeners.add(listener);
      return () => hoverListeners.delete(listener);
    },
    onCursorChange(listener) {
      cursorListeners.add(listener);
      return () => cursorListeners.delete(listener);
    },
    setCursor(position) {
      const anchor = Math.min(Math.max(position, 0), view.state.doc.length);
      view.dispatch({ selection: { anchor }, scrollIntoView: true });
    },
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  };
}

function notifyEditorUpdate(
  update: ViewUpdate,
  changeListeners: ReadonlySet<(change: EditorChange) => void>,
  cursorListeners: ReadonlySet<(change: CursorChange) => void>
) {
  if (update.docChanged) notifyDocumentChange(update, changeListeners);
  if (update.selectionSet || update.docChanged) {
    const change: CursorChange = { position: update.state.selection.main.head };
    for (const listener of cursorListeners) listener(change);
  }
}

function notifyDocumentChange(
  update: ViewUpdate,
  listeners: ReadonlySet<(change: EditorChange) => void>
) {
  const remote = update.transactions.some((transaction) =>
    transaction.effects.some((effect) => effect.is(remoteOrigin))
  );
  if (remote) return;
  const change: EditorChange = {
    content: update.state.doc.toString(),
    origin: 'local',
  };
  for (const listener of listeners) listener(change);
}

function highlightDecorations(view: EditorView, spans: readonly HighlightSpan[]): DecorationSet {
  // Span offsets are UTF-16 code units — CM6's own index space. Clamp to the
  // live document: spans race edits, and the next parse reconciles.
  const docLength = view.state.doc.length;
  const ranges = spans.flatMap((span) => {
    const from = Math.min(span.startIndex, docLength);
    const to = Math.min(span.endIndex, docLength);
    return from < to
      ? [Decoration.mark({ class: syntaxClassFor(span.capture) }).range(from, to)]
      : [];
  });
  return Decoration.set(ranges, true);
}

function hostDecorationSet(view: EditorView, decorations: EditorDecorations): DecorationSet {
  const ranges = [];
  for (const item of decorations.lineBackgrounds ?? []) {
    const line = view.state.doc.line(Math.min(Math.max(item.line, 1), view.state.doc.lines));
    ranges.push(Decoration.line({ class: item.className }).range(line.from));
  }
  for (const item of decorations.underlines ?? []) {
    const from = Math.min(item.from, view.state.doc.length);
    const to = Math.min(item.to, view.state.doc.length);
    if (from < to) ranges.push(Decoration.mark({ class: item.className }).range(from, to));
  }
  return Decoration.set(ranges, true);
}

/** The tooltip data carried alongside the underline paint, clamped the same way. */
function diagnosticSpans(
  decorations: EditorDecorations,
  docLength: number
): readonly DiagnosticSpan[] {
  const spans: DiagnosticSpan[] = [];
  for (const item of decorations.underlines ?? []) {
    if (item.message === undefined || item.message.length === 0) continue;
    spans.push({
      from: Math.min(item.from, docLength),
      to: Math.min(item.to, docLength),
      message: item.message,
    });
  }
  return spans;
}

function gutterMarkers(decorations: EditorDecorations, view: EditorView): RangeSet<GutterMarker> {
  const markers = [];
  for (const item of decorations.gutterTexts ?? []) {
    const line = view.state.doc.line(Math.min(Math.max(item.line, 1), view.state.doc.lines));
    markers.push(new TextMarker(item.text, item.className).range(line.from));
  }
  return RangeSet.of(markers, true);
}

function readMetrics(view: EditorView): LayoutMetrics {
  return {
    lineHeight: view.defaultLineHeight,
    characterWidth: view.defaultCharacterWidth,
    contentWidth: view.contentDOM.scrollWidth,
    contentHeight: view.contentDOM.scrollHeight,
    scrollTop: view.scrollDOM.scrollTop,
    scrollLeft: view.scrollDOM.scrollLeft,
  };
}
