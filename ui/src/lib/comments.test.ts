import { describe, expect, it } from 'vitest';
import { parse } from 'yaml';
import { addComment, editComment, removeComment, commentsOf, findQuote, nthAt } from './comments';
import { splitRaw } from './frontmatter';

const body = '# Pie\n\n## Directions\n\n1. Beat eggs  slightly.\n2. Beat eggs again.\n';
const raw = `---\ntags:\n  - Dessert # sweet\nauthor: Ann Lee\n---\n\n${body}`;
const fm = (r: string) => parse(splitRaw(r)[0]) ?? {};
const note = { id: 'a1', quote: 'Beat eggs slightly.', text: 'Use a fork', by: 'Ann Lee', date: '2026-11-27' };

describe('notes in the recipe file', () => {
  it('adds, edits and removes notes without touching the text or the other properties', () => {
    const one = addComment(raw, note);
    expect(splitRaw(one)[1]).toBe(splitRaw(raw)[1]);
    expect(one).toContain('- Dessert # sweet');
    expect(commentsOf(fm(one))).toEqual([note]);

    const two = addComment(one, { ...note, id: 'b2', quote: 'Beat eggs', nth: 1, text: 'Twice' });
    expect(commentsOf(fm(two)).map((c) => [c.id, c.nth])).toEqual([['a1', undefined], ['b2', 1]]);

    const edited = editComment(two, 'a1', 'Use a whisk', 'Ben Ross', '2026-11-28');
    expect(commentsOf(fm(edited))[0]).toMatchObject({ text: 'Use a whisk', by: 'Ann Lee', edited_by: 'Ben Ross', edited: '2026-11-28' });
    expect(editComment(edited, 'gone', 'x', 'Ben', 'd')).toBe(edited);

    const less = removeComment(edited, 'a1');
    expect(commentsOf(fm(less)).map((c) => c.id)).toEqual(['b2']);
    const none = removeComment(less, 'b2');
    expect(fm(none).comments).toBeUndefined();
    expect(none).toBe(raw);
  });

  it('starts a property block for a note without one', () => {
    const out = addComment(body, { ...note, quote: '' });
    expect(out.startsWith('---\ncomments:\n  - id: a1\n')).toBe(true);
    expect(splitRaw(out)[1]).toBe('\n' + body);
  });

  it('ignores entries that aren’t notes', () => {
    expect(commentsOf({ comments: ['x', { id: 'a' }, { id: 'b', text: 'ok', by: 'Ann', date: 'd', nth: 0 }] })).toEqual([{ id: 'b', quote: '', text: 'ok', by: 'Ann', date: 'd' }]);
    expect(commentsOf({ comments: 'nope' })).toEqual([]);
  });
});

describe('finding the highlighted words', () => {
  const text = 'Beat eggs  slightly.\nBeat eggs again. Beat\neggs (a+b)?';
  it('matches across spaces and line breaks, the nth time', () => {
    expect(findQuote(text, 'Beat eggs slightly.')).toEqual([0, 20]);
    expect(text.slice(...findQuote(text, 'Beat eggs', 2)!)).toBe('Beat\neggs');
    expect(findQuote(text, 'Beat eggs', 9)).toEqual([0, 9]); // fewer than that: the first
    expect(findQuote(text, '(a+b)?')).not.toBeNull();
    expect(findQuote(text, 'Whisk')).toBeNull();
    expect(findQuote(text, '  ')).toBeNull();
  });
  it('counts which time the words appear at a place', () => {
    expect(nthAt(text, 'Beat eggs', 0)).toBe(0);
    expect(nthAt(text, 'Beat eggs', text.indexOf('Beat eggs again'))).toBe(1);
    expect(nthAt(text, 'Beat eggs', text.lastIndexOf('Beat'))).toBe(2);
  });
});
