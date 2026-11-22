import { describe, it, expect } from 'vitest';
import { parse } from 'yaml';
import { stamp, creditedByDefault } from './credit';
import { splitRaw } from './frontmatter';

const fm = (raw: string) => parse(splitRaw(raw)[0]);
const body = '# Pie\n\n## Ingredients\n\n- 1 cup sugar\n';

describe('stamp', () => {
  it('adds an author and the first change to a note without properties', () => {
    const out = stamp(body, { by: 'Ann Romo', date: '2026-11-21', note: 'Created', author: true });
    expect(fm(out)).toEqual({ author: 'Ann Romo', changes: [{ by: 'Ann Romo', date: '2026-11-21', note: 'Created' }] });
    expect(splitRaw(out)[1]).toBe('\n' + body);
  });

  it('turns one author into a list, once, in any letter case', () => {
    const raw = `---\ntags:\n  - Dessert # sweet\nauthor: Kimberley Romo\n---\n\n${body}`;
    const one = stamp(raw, { by: 'Ann Romo', date: '2026-11-21', note: 'Less sugar', author: true });
    expect(fm(one).author).toEqual(['Kimberley Romo', 'Ann Romo']);
    const two = stamp(one, { by: 'ann romo', date: '2026-11-22', note: 'Edited', author: true });
    expect(fm(two).author).toEqual(['Kimberley Romo', 'Ann Romo']);
    expect(two).toContain('- Dessert # sweet'); // comments and order kept
    expect(two.indexOf('tags:')).toBeLessThan(two.indexOf('author:'));
    expect(splitRaw(two)[1]).toBe(splitRaw(raw)[1]);
  });

  it('leaves the author alone unless asked', () => {
    const raw = `---\nauthor: Kimberley Romo\n---\n\n${body}`;
    expect(fm(stamp(raw, { by: 'Ann Romo', date: '2026-11-21', note: 'Typo' })).author).toBe('Kimberley Romo');
  });

  it('keeps who added a recipe', () => {
    const raw = `---\nsource: https://www.food.com\n---\n\n${body}`;
    const added = stamp(raw, { by: 'Ann Romo', date: '2026-11-21', note: 'Imported from food.com', added: true });
    expect(fm(added).added_by).toBe('Ann Romo');
    expect(fm(stamp(added, { by: 'Ben Romo', date: '2026-11-22', note: 'x', added: true })).added_by).toBe('Ann Romo');
  });

  it('merges one person’s changes on the same day', () => {
    let raw = stamp(body, { by: 'Ann Romo', date: '2026-11-21', note: 'Less sugar' });
    raw = stamp(raw, { by: 'Ann Romo', date: '2026-11-21', note: 'Changed prep time' });
    raw = stamp(raw, { by: 'Ann Romo', date: '2026-11-21', note: 'Less sugar' });
    raw = stamp(raw, { by: 'Ben Romo', date: '2026-11-21', note: 'Edited' });
    raw = stamp(raw, { by: 'Ann Romo', date: '2026-11-22', note: 'Edited' });
    expect(fm(raw).changes).toEqual([
      { by: 'Ann Romo', date: '2026-11-21', note: 'Less sugar; Changed prep time' },
      { by: 'Ben Romo', date: '2026-11-21', note: 'Edited' },
      { by: 'Ann Romo', date: '2026-11-22', note: 'Edited' },
    ]);
  });

  it('does nothing without a name', () => {
    expect(stamp(body, { by: ' ', date: '2026-11-21', note: 'x', author: true })).toBe(body);
  });
});

describe('creditedByDefault', () => {
  it('is ticked unless someone else wrote the recipe', () => {
    expect(creditedByDefault({}, 'Ann Romo')).toBe(true);
    expect(creditedByDefault({ author: 'Kimberley Romo' }, 'Ann Romo')).toBe(false);
    expect(creditedByDefault({ author: ['Kimberley Romo', 'ann romo'] }, 'Ann Romo')).toBe(true);
    expect(creditedByDefault({ author: 'Martha Stewart', added_by: 'Ann Romo' }, 'Ann Romo')).toBe(true);
  });
});
