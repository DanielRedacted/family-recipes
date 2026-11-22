import { describe, expect, it } from 'vitest';
import { blockedProteins, madeToday, type Made } from './made';

const at = (y: number, mo: number, d: number, h = 19) => new Date(y, mo - 1, d, h).getTime();
const made = (when: number, tags: string[], path = 'Meat/Chops.md'): Made => ({ at: when / 1000, path, title: 'x', tags });

describe('blockedProteins', () => {
  const sat = at(2026, 11, 21);
  it('blocks a dinner protein through the third day after, not the fourth', () => {
    const log = [made(sat, ['Dinner', 'Pork'])];
    expect(blockedProteins(log, at(2026, 11, 21, 21))).toEqual(['Pork']);
    expect(blockedProteins(log, at(2026, 11, 24, 23))).toEqual(['Pork']);
    expect(blockedProteins(log, at(2026, 11, 25, 0))).toEqual([]);
  });
  it('ignores meals that are not dinners, and blocks every protein of a dinner', () => {
    expect(blockedProteins([made(sat, ['Lunch', 'Chicken'])], sat)).toEqual([]);
    expect(blockedProteins([made(sat, ['dinner', 'chicken', 'Seafood'])], sat)).toEqual(['Chicken', 'Seafood']);
  });
});

it('madeToday is by local calendar day', () => {
  const log = [made(at(2026, 11, 21, 8), ['Dinner'])];
  expect(madeToday(log, 'Meat/Chops.md', at(2026, 11, 21, 23))).toBe(true);
  expect(madeToday(log, 'Meat/Chops.md', at(2026, 11, 22, 0))).toBe(false);
  expect(madeToday(log, 'Other.md', at(2026, 11, 21, 23))).toBe(false);
});
