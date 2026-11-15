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

import { lineKind, setLines } from './mdCommands';

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
