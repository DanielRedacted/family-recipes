import { describe, expect, it } from 'vitest';
import { daysOf, eventsOn, history, keyOf, timeOf, type CalEvent } from './calendar';
import type { Entry } from './types';

const s = (y: number, mo: number, d: number, h = 0, mi = 0) => new Date(y, mo - 1, d, h, mi).getTime() / 1000;
const ev = (title: string, start: number, end: number, all_day = false): CalEvent => ({ title, start, end, all_day, calendar: 'Home', location: null });
const note = (path: string, fm: Record<string, unknown>) => ({ path, kind: 'note', title: path.replace('.md', ''), frontmatter: fm }) as unknown as Entry;

describe('daysOf', () => {
  it('a week runs Sunday to Saturday', () => {
    const w = daysOf('week', new Date(2026, 10, 25)); // Wednesday 25 Nov 2026
    expect(w.map(keyOf)).toEqual(['2026-11-22', '2026-11-23', '2026-11-24', '2026-11-25', '2026-11-26', '2026-11-27', '2026-11-28']);
  });
  it('a month covers whole weeks, and stays on local midnights across a clock change', () => {
    const m = daysOf('month', new Date(2026, 10, 15)); // November 2026 (US clocks change on the 1st)
    expect(keyOf(m[0])).toBe('2026-11-01');
    expect(keyOf(m[m.length - 1])).toBe('2026-12-05');
    expect(m.length % 7).toBe(0);
    expect(m.every((d) => d.getHours() === 0)).toBe(true);
    expect(new Set(m.map(keyOf)).size).toBe(m.length);
  });
});

describe('events', () => {
  const day = new Date(2026, 10, 22);
  it('an event shows on every day it touches, and not on the day it ends at midnight', () => {
    const trip = ev('Trip', s(2026, 11, 21, 18), s(2026, 11, 23, 0));
    expect(eventsOn([trip], new Date(2026, 10, 21))).toHaveLength(1);
    expect(eventsOn([trip], day)).toHaveLength(1);
    expect(eventsOn([trip], new Date(2026, 10, 23))).toHaveLength(0);
    expect(timeOf(trip, day)).toBe('All day');
    expect(timeOf(trip, new Date(2026, 10, 21))).toMatch(/^From /);
  });
  it('all-day events say so', () => {
    expect(timeOf(ev('Birthday', s(2026, 11, 22), s(2026, 11, 22, 23, 59), true), day)).toBe('All day');
  });
});

describe('history', () => {
  it('sorts meals, additions and changes onto their days', () => {
    const files = [
      note('Pie.md', { added_by: 'Ann Smith', changes: [{ by: 'Ann Smith', date: '2026-11-20', note: 'Imported from a website' }, { by: 'Bob Jones', date: '2026-11-22', note: 'Fixed the oven temperature' }] }),
      note('Soup.md', { changes: [{ by: 'Cy', date: '2026-11-22', note: 'Created; Added a photo' }] }),
      note('Old.md', { changes: [{ by: 'Dee', date: '2026-11-22', note: 'Tidied' }] }), // its first entry, but not its arrival
      note('Bad.md', { changes: 'not a list' }),
    ];
    const made = [{ at: s(2026, 11, 22, 19), path: 'Pie.md', title: 'Pie', tags: [], by: 'Ann', assumed: true }];
    const h = history(made, files);
    expect(h.get('2026-11-20')?.map((i) => i.kind)).toEqual(['added']);
    expect(h.get('2026-11-22')?.map((i) => i.kind)).toEqual(['cooked', 'added', 'changed', 'changed']);
    const last = h.get('2026-11-22')!.filter((i) => i.kind === 'changed');
    expect(last.map((i) => i.kind === 'changed' && i.title)).toEqual(['Pie', 'Old']);
  });
});
