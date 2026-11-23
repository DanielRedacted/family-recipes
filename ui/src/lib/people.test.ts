import { describe, it, expect } from 'vitest';
import { creditsOf, directory, findPerson, initials, personHash } from './people';
import type { Entry, Person } from './types';

const note = (path: string, frontmatter: Record<string, unknown>): Entry => ({ path, kind: 'note', title: path, frontmatter } as unknown as Entry);
const files = [
  note('Pie.md', { author: 'Kimberley Romo', changes: [{ by: 'Ann Romo', date: '2026-11-01', note: 'Typo' }] }),
  note('Soup.md', { author: ['Mamaw', 'kimberley  romo'] }),
  note('Bread.md', { added_by: 'Ann Romo', author: 'Betty Crocker' }),
  note('Tea.md', { changes: [{ by: 'Ann Smith', date: '2026-01-01', note: 'Created' }] }),
  { path: 'scan.pdf', kind: 'pdf', title: 'scan', frontmatter: { author: 'Ann Romo' } } as unknown as Entry,
];
const ann: Person = { id: 'a', name: 'Ann', full_name: 'Ann Romo', former: ['Ann Smith'] };

describe('creditsOf', () => {
  it('sorts each recipe into wrote, added or edited, once, in any letter case and spacing', () => {
    const c = creditsOf(files, ['Kimberley Romo']);
    expect(c.wrote.map((f) => f.path)).toEqual(['Pie.md', 'Soup.md']);
    const a = creditsOf(files, ['Ann Romo', 'Ann', 'Ann Smith']);
    expect([a.wrote, a.added, a.edited].map((l) => l.map((f) => f.path))).toEqual([[], ['Bread.md'], ['Pie.md', 'Tea.md']]);
  });
});

describe('directory', () => {
  it('lists people who sign in with every name they were credited under, then everyone else', () => {
    const { family, others } = directory(files, [ann]);
    expect(family).toMatchObject([{ name: 'Ann Romo', recipes: 3 }]);
    expect(others).toEqual([{ name: 'Kimberley Romo', recipes: 2 }, { name: 'Betty Crocker', recipes: 1 }, { name: 'Mamaw', recipes: 1 }]);
  });
});

describe('pages', () => {
  it('finds a person by any of their names and round-trips names through the hash', () => {
    expect(findPerson([ann], 'ann smith')).toBe(ann);
    expect(findPerson([ann], 'Mamaw')).toBeUndefined();
    expect(decodeURIComponent(personHash('Julia DeLucci / Betty').slice('#/people/'.length))).toBe('Julia DeLucci / Betty');
    expect([initials('Kimberley Romo'), initials('Mamaw'), initials('Sportello, Boston'), initials('Élise de la Tour')]).toEqual(['KR', 'M', 'SB', 'ÉD']);
  });
});
