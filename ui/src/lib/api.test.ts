import { describe, expect, it } from 'vitest';
import { inBook, settleBook, BOOK, bookKey, fileUrl } from './api';
import { inCookbook, madeToday } from './made';

describe('cookbooks', () => {
  it('names the cookbook on every API address, once', () => {
    expect(inBook('/api/vault', 'b2')).toBe('/api/vault?book=b2');
    expect(inBook('/api/made/5?path=Pie.md', 'b2')).toBe('/api/made/5?path=Pie.md&book=b2');
    expect(inBook('/api/vault?book=1', 'b2')).toBe('/api/vault?book=1');
    expect(inBook('/icon.svg', 'b2')).toBe('/icon.svg');
    expect(inBook('/api/vault', '')).toBe('/api/vault');
  });

  it('settles on the stored choice, or the first when it is gone', () => {
    expect(settleBook(['1', 'b2'])).toBe(true); // nothing chosen yet: the first, without a reload
    expect(BOOK).toBe('1');
    expect(bookKey('tabs')).toBe('tabs'); // the first cookbook keeps the keys from before cookbooks
    expect(fileUrl('Pies/Pumpkin Pie.md')).toBe('/api/files/Pies/Pumpkin%20Pie.md?book=1');
    expect(settleBook(['b2'])).toBe(false); // removed: reload into the first left
    expect(BOOK).toBe('b2');
    expect(bookKey('tabs')).toBe('tabs@b2');
  });

  it('keeps one meal history, but "made today" is per cookbook', () => {
    const now = Date.now();
    const made = [{ at: Math.floor(now / 1000), book: 'b2', path: 'Pie.md', title: 'Pie', tags: [] }];
    expect(madeToday(made, 'Pie.md', now, false, 'b2')).toBe(true);
    expect(madeToday(made, 'Pie.md', now, false, '1')).toBe(false);
    expect(inCookbook({ ...made[0], book: undefined }, '1')).toBe(true); // a meal from before cookbooks
  });

  // a file address built by hand would reach the device's first cookbook, not the tab's (reading one
  // back, as the activity list does, is fine)
  it('builds file addresses only through api.ts', () => {
    const sources = import.meta.glob(['../**/*.ts', '../**/*.svelte', '!../**/*.test.ts', '!./api.ts'], { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
    expect(Object.keys(sources).length).toBeGreaterThan(20);
    const raw = Object.entries(sources).filter(([, text]) => /['"]\/api\/files\/['"]\s*\+|`\/api\/files\/\$\{/.test(text)).map(([f]) => f);
    expect(raw).toEqual([]);
  });
});
