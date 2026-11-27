// Recipe credits, written into the recipe's frontmatter (docs/FORMAT.md):
//   added_by  who imported it (set once, never replaced)
//   author    who wrote it; one name, or a list when several people did
//   changes   [{ by, date, note }], one entry per person per day
// Kept free of Svelte so vitest can run it.
import { editProps } from './frontmatter';

export interface Stamp {
  /** the person's full name */
  by: string;
  /** YYYY-MM-DD, local */
  date: string;
  note: string;
  /** also list `by` among the authors */
  author?: boolean;
  /** an import: set `added_by` if the recipe has none */
  added?: boolean;
}

const same = (a: unknown, b: string) => typeof a === 'string' && a.trim().toLowerCase() === b.trim().toLowerCase();

/** The `author` property as a list of names. "A / B", "A & B" and "A and B" are two people; commas are not
 *  ("Luigi's, Boston" is one restaurant). */
export function authorsOf(v: unknown): string[] {
  const list = typeof v === 'string' ? [v] : Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
  return list.flatMap((s) => s.split(/\s+(?:\/|&|\+|and)\s+/i)).map((s) => s.trim()).filter(Boolean);
}

/** Should "Credit me as an author" start ticked? Yes unless someone else is the recipe's author. */
export function creditedByDefault(fm: Record<string, unknown> | null | undefined, name: string): boolean {
  const authors = authorsOf(fm?.author);
  return !authors.length || authors.some((a) => same(a, name)) || same(fm?.added_by, name);
}

/** Record a change (and optionally an author or who added it) in the recipe's frontmatter. */
export function stamp(raw: string, s: Stamp): string {
  if (!s.by.trim()) return raw;
  return editProps(raw, (doc) => {
    if (s.added && !doc.get('added_by')) doc.set('added_by', s.by);
    if (s.author) {
      const authors = authorsOf(doc.toJS()?.author);
      if (!authors.some((a) => same(a, s.by))) doc.set('author', authors.length ? [...authors, s.by] : s.by);
    }
    const prev = doc.toJS()?.changes;
    const list: { by: string; date: string; note: string }[] = Array.isArray(prev) ? prev.filter((c) => c && typeof c === 'object') : [];
    const last = list[list.length - 1];
    if (last && same(last.by, s.by) && String(last.date) === s.date) {
      const notes = String(last.note ?? '').split('; ').filter(Boolean);
      if (!notes.includes(s.note)) last.note = [...notes, s.note].join('; ');
    } else list.push({ by: s.by, date: s.date, note: s.note });
    doc.set('changes', list);
  });
}

/** Today as `YYYY-MM-DD` in local time. */
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}
