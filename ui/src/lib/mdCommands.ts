// Formatting commands for the editor toolbar. Each works on an EditorState and returns the
// transaction to apply, so they can be tested without a browser.
import { EditorSelection, type EditorState, type Line, type TransactionSpec } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';

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

export type LineKind = 'p' | 'h1' | 'h2' | 'h3' | 'bullet' | 'number' | 'task' | 'quote';
const PREFIX: Record<LineKind, string> = { p: '', h1: '# ', h2: '## ', h3: '### ', bullet: '- ', number: '1. ', task: '- [ ] ', quote: '> ' };
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

/** The selected lines, in order. */
function selectedLines(state: EditorState): number[] {
  const lines = new Set<number>();
  for (const r of state.selection.ranges) {
    for (let l = state.doc.lineAt(r.from).number; l <= state.doc.lineAt(r.to).number; l++) lines.add(l);
  }
  return [...lines].sort((a, b) => a - b);
}

/** Make every selected line body text or a heading (the "text type" menu; never toggles off). */
export function setLines(state: EditorState, kind: 'p' | 'h1' | 'h2' | 'h3'): TransactionSpec {
  return {
    changes: selectedLines(state).map((n) => {
      const line = state.doc.line(n);
      const m = ANY.exec(line.text)!;
      const from = line.from + m[1].length;
      return { from, to: from + (m[2]?.length ?? 0), insert: PREFIX[kind] };
    }),
  };
}

/** Text type of the line the cursor is on, for the toolbar menu. */
export function lineKind(state: EditorState): 'p' | 'h1' | 'h2' | 'h3' {
  const m = /^\s*(#{1,6}) /.exec(state.doc.lineAt(state.selection.main.head).text);
  if (!m) return 'p';
  return m[1].length === 1 ? 'h1' : m[1].length === 2 ? 'h2' : 'h3';
}

// ---------- tables ----------

/** Cell content ranges of one table row (offsets in `text`), trimmed; an empty cell is a point. */
export function cellRanges(text: string): { from: number; to: number }[] {
  const pipes: number[] = [];
  for (let i = 0; i < text.length; i++) if (text[i] === '|' && text[i - 1] !== '\\') pipes.push(i);
  const bounds = [-1, ...pipes, text.length];
  const out: { from: number; to: number }[] = [];
  for (let i = 0; i < bounds.length - 1; i++) {
    const a = bounds[i] + 1, b = bounds[i + 1];
    const t = text.slice(a, b);
    if ((i === 0 || i === bounds.length - 2) && !t.trim()) continue; // outside the outer pipes
    const lead = t.length - t.trimStart().length;
    out.push(t.trim() ? { from: a + lead, to: a + lead + t.trim().length } : { from: Math.min(a + 1, b), to: Math.min(a + 1, b) });
  }
  return out;
}

const cellsOf = (text: string) => cellRanges(text).map((r) => text.slice(r.from, r.to));
const joinRow = (cells: string[]) => '| ' + cells.join(' | ') + ' |';

export interface TableInfo { first: number; last: number; row: number; col: number; cols: number }

/** The table around the cursor: its line numbers, the cursor's row (0 = header, 1 = separator) and column. */
export function tableInfo(state: EditorState, pos = state.selection.main.head): TableInfo | null {
  const line = state.doc.lineAt(pos);
  let table: { from: number; to: number } | null = null;
  syntaxTree(state).iterate({ from: line.from, to: line.to, enter: (n) => { if (n.name === 'Table') { table = { from: n.from, to: n.to }; return false; } } });
  if (!table) return null;
  const { from, to } = table as { from: number; to: number };
  const first = state.doc.lineAt(from).number;
  const last = state.doc.lineAt(to).number;
  const ranges = cellRanges(line.text);
  const off = pos - line.from;
  let col = ranges.findIndex((r, i) => off <= r.to || (ranges[i + 1] && off < ranges[i + 1].from));
  if (col < 0) col = ranges.length - 1;
  return { first, last, row: line.number - first, col: Math.max(0, col), cols: cellsOf(state.doc.line(first).text).length };
}

/** Insert an empty 2 × 2 table (header + one row) and put the cursor in its first cell. */
export function insertTable(state: EditorState): TransactionSpec {
  const line = state.doc.lineAt(state.selection.main.head);
  const blank = !line.text.trim();
  const prevText = line.number > 1 ? state.doc.line(line.number - 1).text : '';
  const before = blank ? (prevText.trim() ? '\n' : '') : '\n\n';
  const nextText = line.number < state.doc.lines ? state.doc.line(line.number + 1).text : '';
  const after = nextText.trim() ? '\n' : '';
  const table = '|  |  |\n| --- | --- |\n|  |  |';
  const at = blank ? line.from : line.to;
  return {
    changes: { from: at, to: blank ? line.to : line.to, insert: before + table + after },
    selection: { anchor: at + before.length + 2 },
  };
}

function rewrite(state: EditorState, t: TableInfo, lines: string[], cursor: { row: number; col: number }): TransactionSpec {
  const from = state.doc.line(t.first).from;
  const to = state.doc.line(t.last).to;
  const text = lines.join('\n');
  let start = from;
  for (let i = 0; i < cursor.row; i++) start += lines[i].length + 1;
  const r = cellRanges(lines[cursor.row])[Math.min(cursor.col, cellRanges(lines[cursor.row]).length - 1)];
  return { changes: { from, to, insert: text }, selection: { anchor: start + (r?.from ?? 0), head: start + (r?.to ?? 0) } };
}

/** Add or delete a row or column at the cursor. Cell text is kept; spacing is normalised. */
export function tableOp(state: EditorState, op: 'row+' | 'row-' | 'col+' | 'col-'): TransactionSpec | null {
  const t = tableInfo(state);
  if (!t) return null;
  const rows: string[][] = [];
  for (let n = t.first; n <= t.last; n++) rows.push(cellsOf(state.doc.line(n).text));
  const body = (i: number) => i >= 2;
  let { row, col } = t;
  if (op === 'row+') {
    const at = Math.max(row, 1) + 1;
    rows.splice(at, 0, Array(t.cols).fill(''));
    row = at; col = 0;
  } else if (op === 'row-') {
    if (!body(row)) return null;
    rows.splice(row, 1);
    row = Math.min(row, rows.length - 1);
    if (row === 1) row = 0;
  } else if (op === 'col+') {
    rows.forEach((r, i) => r.splice(col + 1, 0, i === 1 ? '---' : ''));
    col += 1;
  } else {
    if (t.cols < 2) return null;
    rows.forEach((r) => r.splice(col, 1));
    col = Math.min(col, t.cols - 2);
  }
  if (row === 1) row = 0;
  return rewrite(state, t, rows.map(joinRow), { row, col });
}

/** Tab / Shift-Tab / Enter in a table: select the next cell, adding a row after the last one. */
export function tableMove(state: EditorState, dir: 1 | -1 | 'down'): TransactionSpec | null {
  const t = tableInfo(state);
  if (!t) return null;
  const rowLines = [];
  for (let n = t.first; n <= t.last; n++) if (n !== t.first + 1) rowLines.push(n);
  let ri = Math.max(0, rowLines.indexOf(t.first + t.row));
  let col = t.col;
  const count = (n: number) => cellRanges(state.doc.line(n).text).length;
  if (dir === 1) { if (col + 1 < count(rowLines[ri])) col++; else { ri++; col = 0; } }
  else if (dir === -1) { if (col > 0) col--; else if (ri > 0) { ri--; col = count(rowLines[ri]) - 1; } else return { selection: state.selection }; }
  else ri++;
  if (ri >= rowLines.length) {
    const end = state.doc.line(t.last).to;
    const row = '\n' + joinRow(Array(t.cols).fill(''));
    const r = cellRanges(row.slice(1))[Math.min(col, t.cols - 1)];
    return { changes: { from: end, insert: row }, selection: { anchor: end + 1 + r.from } };
  }
  const line: Line = state.doc.line(rowLines[ri]);
  const rs = cellRanges(line.text);
  const r = rs[Math.min(col, rs.length - 1)];
  return { selection: { anchor: line.from + r.from, head: line.from + r.to }, scrollIntoView: true };
}
