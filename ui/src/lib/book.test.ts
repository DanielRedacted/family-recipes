import { describe, expect, it, vi } from 'vitest';
import type { Entry } from './types';

// the app's state wires itself to the window when loaded; the order needs only dirOf (copied from it)
vi.mock('./state.svelte', () => ({ app: {}, prefs: {}, dirOf: (p: string) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/')) : '') }));
const { bookOrder } = await import('./book');

const note = (path: string, title: string, kind: Entry['kind'] = 'note'): Entry =>
  ({ path, title, kind, size: 0, mtime: 0, etag: '', body_line: 0 });

describe('bookOrder', () => {
  it('makes a chapter per folder, by name, with recipes by title', () => {
    const order = bookOrder([
      note('Pies/pumpkin.md', 'Pumpkin Pie'),
      note('Pies/Apple.md', 'apple pie'),
      note('Breakfast Breads/Scones.md', 'Scones'),
      note('Breads/Sourdough/Starter.md', 'Starter'),
      note('Breads/Rolls.md', 'Rolls'),
      note('Cookies/Cookie 10.md', 'Cookie 10'),
      note('Cookies/Cookie 9.md', 'Cookie 9'),
      note('Kitchen notes.md', 'Kitchen notes'),
      note('attachments/originals/Pie.md', 'Pie', 'other'),
      note('attachments/scan.md', 'Scan'),
      note('Pies/crust.pdf', 'crust', 'pdf'),
    ]);
    expect(order.map((c) => c.name)).toEqual(['Breads', 'Breads / Sourdough', 'Breakfast Breads', 'Cookies', 'Pies', 'Other recipes']);
    expect(order.find((c) => c.name === 'Pies')!.recipes.map((r) => r.title)).toEqual(['apple pie', 'Pumpkin Pie']);
    expect(order.find((c) => c.name === 'Cookies')!.recipes.map((r) => r.title)).toEqual(['Cookie 9', 'Cookie 10']);
  });
});
