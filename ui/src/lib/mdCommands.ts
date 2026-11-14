// Formatting commands for the editor toolbar. Each works on an EditorState and returns the
// transaction to apply, so they can be tested without a browser.
import { EditorSelection, type EditorState, type TransactionSpec } from '@codemirror/state';

/** Toggle `**bold**` / `*italic*` around each selection (or insert a pair at the cursor). */
export function toggleWrap(state: EditorState, m: string): TransactionSpec {
  return state.changeByRange((r) => {
    const before = state.sliceDoc(r.from - m.length, r.from);
    const after = state.sliceDoc(r.to, r.to + m.length);
    if (before === m && after === m) {
      return {
        changes: [{ from: r.from - m.length, to: r.from }, { from: r.to, to: r.to + m.length }],
        range: EditorSelection.range(r.from - m.length, r.to - m.length),
      };
    }
    const text = state.sliceDoc(r.from, r.to);
    if (text.length > 2 * m.length && text.startsWith(m) && text.endsWith(m)) {
      return { changes: { from: r.from, to: r.to, insert: text.slice(m.length, -m.length) }, range: EditorSelection.range(r.from, r.to - 2 * m.length) };
    }
    return {
      changes: [{ from: r.from, insert: m }, { from: r.to, insert: m }],
      range: EditorSelection.range(r.from + m.length, r.to + m.length),
    };
  });
}

export type LineKind = 'h2' | 'h3' | 'bullet' | 'number' | 'task' | 'quote';
const PREFIX: Record<LineKind, string> = { h2: '## ', h3: '### ', bullet: '- ', number: '1. ', task: '- [ ] ', quote: '> ' };
// any existing block prefix (heading, list, task, quote) — replaced rather than stacked
const ANY = /^(\s*)(#{1,6} |[-*+] \[[ xX]\] |[-*+] |\d+[.)] |> )?/;

/** Toggle a line prefix on every selected line; numbered lists count 1, 2, 3… */
export function toggleLines(state: EditorState, kind: LineKind): TransactionSpec {
  const lines = new Set<number>();
  for (const r of state.selection.ranges) {
    for (let l = state.doc.lineAt(r.from).number; l <= state.doc.lineAt(r.to).number; l++) lines.add(l);
  }
  const nums = [...lines].sort((a, b) => a - b);
  const has = (t: string) => {
    const p = ANY.exec(t)![2] ?? '';
    return kind === 'number' ? /^\d+[.)] $/.test(p) : kind === 'bullet' ? /^[-*+] $/.test(p) : p === PREFIX[kind];
  };
  const allHave = nums.every((n) => has(state.doc.line(n).text));
  const changes = nums.map((n, i) => {
    const line = state.doc.line(n);
    const m = ANY.exec(line.text)!;
    const from = line.from + m[1].length;
    const to = from + (m[2]?.length ?? 0);
    const insert = allHave ? '' : kind === 'number' ? `${i + 1}. ` : PREFIX[kind];
    return { from, to, insert };
  });
  return { changes };
}
