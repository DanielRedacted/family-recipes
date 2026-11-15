import { describe, expect, it } from 'vitest';
import { EditorSelection, EditorState } from '@codemirror/state';
import { toggleLines, toggleWrap } from './mdCommands';

const run = (doc: string, from: number, to: number, f: (s: EditorState) => any) => {
  const s = EditorState.create({ doc, selection: EditorSelection.single(from, to), extensions: EditorState.allowMultipleSelections.of(true) });
  return s.update(f(s)).state;
};

describe('toggleWrap', () => {
  it('wraps, unwraps, and inserts a pair at the cursor', () => {
    const a = run('mix well', 0, 3, (s) => toggleWrap(s, '**'));
    expect(a.doc.toString()).toBe('**mix** well');
    const b = a.update(toggleWrap(a, '**')).state;
    expect(b.doc.toString()).toBe('mix well');
    expect(run('ab', 1, 1, (s) => toggleWrap(s, '*')).doc.toString()).toBe('a**b');
    expect(run('**x**', 0, 5, (s) => toggleWrap(s, '**')).doc.toString()).toBe('x');
  });
});

describe('toggleLines', () => {
  it('adds, replaces and removes block prefixes', () => {
    const doc = 'flour\nsugar\n- eggs';
    expect(run(doc, 0, 11, (s) => toggleLines(s, 'bullet')).doc.toString()).toBe('- flour\n- sugar\n- eggs');
    expect(run('- a\n- b', 0, 7, (s) => toggleLines(s, 'bullet')).doc.toString()).toBe('a\nb');
    expect(run('- a\n- b', 0, 7, (s) => toggleLines(s, 'number')).doc.toString()).toBe('1. a\n2. b');
    expect(run('Directions', 3, 3, (s) => toggleLines(s, 'h2')).doc.toString()).toBe('## Directions');
    expect(run('### Crust', 3, 3, (s) => toggleLines(s, 'h2')).doc.toString()).toBe('## Crust');
    expect(run('- milk', 2, 2, (s) => toggleLines(s, 'task')).doc.toString()).toBe('- [ ] milk');
    expect(run('  - nested', 4, 4, (s) => toggleLines(s, 'quote')).doc.toString()).toBe('  > nested');
  });
});

import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { ensureSyntaxTree } from '@codemirror/language';
import { insertTable, lineKind, setLines, tableInfo, tableMove, tableOp } from './mdCommands';

const md = (doc: string, at: number) => {
  const s = EditorState.create({ doc, selection: EditorSelection.single(at), extensions: markdown({ base: markdownLanguage }) });
  ensureSyntaxTree(s, s.doc.length, 5000);
  return s;
};
const apply = (s: EditorState, spec: any) => { const n = s.update(spec).state; ensureSyntaxTree(n, n.doc.length, 5000); return n; };

describe('text type', () => {
  it('sets body, title and headings', () => {
    expect(run('## Crust', 3, 3, (s) => setLines(s, 'p')).doc.toString()).toBe('Crust');
    expect(run('Crust', 0, 0, (s) => setLines(s, 'h1')).doc.toString()).toBe('# Crust');
    expect(run('# Crust', 2, 2, (s) => setLines(s, 'h1')).doc.toString()).toBe('# Crust'); // never toggles off
    expect(run('- milk', 2, 2, (s) => setLines(s, 'h3')).doc.toString()).toBe('### milk');
    expect(lineKind(run('a\n## b', 5, 5, () => ({})))).toBe('h2');
    expect(lineKind(run('plain', 1, 1, () => ({})))).toBe('p');
  });
});

describe('tables', () => {
  const T = '| Size | Flour | Water |\n| --- | --- | --- |\n| 1 | 2 cups | 1 cup |\n| 2 | 4 cups | 2 cups |';
  it('finds the cell under the cursor', () => {
    const s = md(T, T.indexOf('4 cups') + 1);
    expect(tableInfo(s)).toMatchObject({ first: 1, last: 4, row: 3, col: 1, cols: 3 });
    expect(tableInfo(md('para\n\n' + T, 2))).toBeNull();
  });
  it('inserts a table that parses as one', () => {
    let s = md('Notes', 5);
    s = apply(s, insertTable(s));
    expect(s.doc.toString()).toBe('Notes\n\n|  |  |\n| --- | --- |\n|  |  |');
    expect(tableInfo(s)).toMatchObject({ row: 0, col: 0, cols: 2 });
  });
  it('adds and removes rows and columns, keeping cell text', () => {
    const at = (s: EditorState) => s.sliceDoc(s.selection.main.from, s.selection.main.to);
    let s = md(T, T.indexOf('2 cups'));
    s = apply(s, tableOp(s, 'col+'));
    expect(s.doc.line(1).text).toBe('| Size | Flour |  | Water |');
    expect(s.doc.line(2).text).toBe('| --- | --- | --- | --- |');
    expect(s.doc.line(3).text).toBe('| 1 | 2 cups |  | 1 cup |');
    s = apply(s, tableOp(s, 'col-'));
    expect(s.doc.toString()).toBe(T);
    s = apply(s, tableOp(s, 'row+'));
    expect(s.doc.line(4).text).toBe('|  |  |  |');
    expect(s.doc.line(5).text).toBe('| 2 | 4 cups | 2 cups |');
    s = apply(s, tableOp(s, 'row-'));
    expect(s.doc.toString()).toBe(T);
    s = md(T, 2); // header row: rows can't be deleted there
    expect(tableOp(s, 'row-')).toBeNull();
    s = md(T, T.indexOf('Water'));
    s = apply(s, tableMove(s, 1)!);
    expect(at(s)).toBe('1'); // wraps to the next row, skipping the separator
    s = apply(s, tableMove(s, -1)!);
    expect(at(s)).toBe('Water');
    s = md(T, T.length - 2);
    s = apply(s, tableMove(s, 1)!);
    expect(s.doc.line(5).text).toBe('|  |  |  |'); // Tab after the last cell adds a row
    expect(tableInfo(s)).toMatchObject({ row: 4, col: 0 });
  });
});
