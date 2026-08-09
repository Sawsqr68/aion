/**
 * The single mapping from a tree-sitter capture name (`comment.documentation`)
 * to the stylesheet class the console colours (`awl-syntax-comment-documentation`).
 *
 * Both the CodeMirror editor decorations and the guide's static example
 * rendering build their class names here, so the two surfaces cannot drift
 * apart from the `.awl-syntax-*` rules in `src/index.css` — one grammar, one
 * query, one class vocabulary.
 */
export function syntaxClassFor(capture: string): string {
  return `awl-syntax-${capture.replaceAll('.', '-').replaceAll(/[^a-zA-Z0-9_-]/g, '')}`;
}
