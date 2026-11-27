// The Calendar page and the home page's Today list: the Mac's calendar events (server:
// crates/core/src/calendar.rs, read-only, off unless switched on) and the app's own history: meals
// cooked (made.ts, guesses included) and recipes added or changed (the `changes` credits, credit.ts).
// Days are local `YYYY-MM-DD` keys built from the browser's clock, so a dated credit never shifts a day.
// Kept free of Svelte so vitest can run it.
import { api } from './api';
import type { Made } from './made';
import type { Entry } from './types';

export interface CalEvent { title: string; start: number; end: number; all_day: boolean; calendar: string; location: string | null }

export type Item =
  | { kind: 'event'; ev: CalEvent }
  | { kind: 'cooked'; made: Made }
  | { kind: 'added'; path: string; title: string; by: string }
  | { kind: 'changed'; path: string; title: string; by: string; note: string };

/** Weeks start on Sunday (Intl.Locale's weekInfo is missing from Safari 15). */
export const WEEK_START = 0;

const pad = (n: number) => String(n).padStart(2, '0');
/** A local day's key. */
export const keyOf = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
/** Local midnight `n` days after `d` (by calendar day, so daylight-saving days are 23 or 25 hours). */
export const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
export const startOfDay = (d: Date) => addDays(d, 0);
export const startOfWeek = (d: Date) => addDays(d, -((d.getDay() - WEEK_START + 7) % 7));

/** The days a view shows: one week, or the whole weeks covering a month. */
export function daysOf(view: 'week' | 'month', anchor: Date): Date[] {
  const first = view === 'week' ? startOfWeek(anchor) : startOfWeek(new Date(anchor.getFullYear(), anchor.getMonth(), 1));
  const last = view === 'week' ? addDays(first, 6) : addDays(startOfWeek(new Date(anchor.getFullYear(), anchor.getMonth() + 1, 0)), 6);
  const out: Date[] = [];
  for (let d = first; d <= last; d = addDays(d, 1)) out.push(d);
  return out;
}

/** The Mac's events between two local midnights; [] when the calendar is off, signed out or offline. */
export async function fetchEvents(from: Date, to: Date): Promise<CalEvent[]> {
  try { return await api<CalEvent[]>('GET', `/api/calendar?from=${Math.floor(from.getTime() / 1000)}&to=${Math.floor(to.getTime() / 1000)}`); } catch { return []; }
}

/** How often a calendar page left on screen asks again (a kitchen tablet; also catches midnight). */
export const REFRESH_MS = 15 * 60_000;

/** Call `load` now, when the page comes back to the front (another tab or app was in front), and every
 * `REFRESH_MS` while it stays on screen. Returns the clean-up. Events change rarely, so these natural
 * moments are enough; `load` itself skips the work while the page is hidden. */
export function refreshAtNaturalTimes(load: () => void): () => void {
  load();
  const t = setInterval(load, REFRESH_MS);
  const back = () => { if (document.visibilityState === 'visible') load(); };
  document.addEventListener('visibilitychange', back);
  window.addEventListener('focus', back); // the Mac app's window coming to the front
  return () => { clearInterval(t); document.removeEventListener('visibilitychange', back); window.removeEventListener('focus', back); };
}

/** Events that touch the local day starting at `d` (a several-day event shows on each of its days). */
export const eventsOn = (events: CalEvent[], d: Date) => {
  const from = d.getTime() / 1000, to = addDays(d, 1).getTime() / 1000;
  return events.filter((e) => e.start < to && e.end > from);
};

const same = (a: unknown, b: string) => typeof a === 'string' && a.trim().toLowerCase() === b.trim().toLowerCase();

/** The app's history by day key: meals (by time), then recipes added, then changes. */
export function history(made: Made[], files: Entry[]): Map<string, Item[]> {
  const out = new Map<string, Item[]>();
  const put = (k: string, it: Item) => { const l = out.get(k); if (l) l.push(it); else out.set(k, [it]); };
  for (const m of [...made].sort((a, b) => a.at - b.at)) put(keyOf(new Date(m.at * 1000)), { kind: 'cooked', made: m });
  const changed: [string, Item][] = [];
  for (const f of files) {
    if (f.kind !== 'note' || !Array.isArray(f.frontmatter?.changes)) continue;
    const addedBy = f.frontmatter?.added_by;
    (f.frontmatter.changes as unknown[]).forEach((c: any, i) => {
      if (!c || typeof c !== 'object' || !/^\d{4}-\d{2}-\d{2}/.test(String(c.date ?? ''))) return;
      const k = String(c.date).slice(0, 10), by = String(c.by ?? '').trim(), note = String(c.note ?? '').trim();
      // the first entry is the recipe arriving: made in the app ("Created") or imported by `added_by`
      const added = i === 0 && (note.split('; ')[0] === 'Created' || same(addedBy, by));
      if (added) put(k, { kind: 'added', path: f.path, title: f.title, by });
      else changed.push([k, { kind: 'changed', path: f.path, title: f.title, by, note }]);
    });
  }
  for (const [k, it] of changed) put(k, it);
  return out;
}

/** "9:00 AM – 10:30 AM", "All day", or for a several-day event the part on day `d`. */
export function timeOf(e: CalEvent, d: Date): string {
  if (e.all_day) return 'All day';
  const t = (s: number) => new Date(s * 1000).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  const from = d.getTime() / 1000, to = addDays(d, 1).getTime() / 1000;
  const before = e.start < from, after = e.end >= to; // an event ending at midnight fills its last day
  if (before && after) return 'All day';
  if (before) return `Until ${t(e.end)}`;
  if (after && e.end > to) return `From ${t(e.start)}`;
  return e.end > e.start ? `${t(e.start)} – ${t(e.end)}` : t(e.start);
}
