// People pages: who is in the book and what they gave it. Credits are read from the recipes
// themselves (`author`, `added_by`, `changes`; see credit.ts), never from the accounts, so someone
// who never signed in (or whose account was removed) still has a page. Kept free of Svelte so
// vitest can run it.
import { authorsOf } from './credit';
import type { Entry, Person } from './types';

export interface Credits { wrote: Entry[]; added: Entry[]; edited: Entry[] }
export interface Listed { name: string; person?: Person; recipes: number }

const key = (s: string) => s.trim().replace(/\s+/g, ' ').toLowerCase();

/** Every name a person's recipes may be credited under. */
export const namesOf = (p: Person) => [p.full_name, p.name, ...(p.former ?? [])].filter((n) => n?.trim());

/** The name a person's page goes by: the one written on their recipes. */
export const nameOnRecipes = (p: Person) => p.full_name || p.name;

export const personHash = (name: string) => '#/people/' + encodeURIComponent(name.trim());

const editorsOf = (fm: Record<string, unknown> | null | undefined): string[] =>
  (Array.isArray(fm?.changes) ? fm.changes : []).map((c: any) => c?.by).filter((b): b is string => typeof b === 'string' && !!b.trim());

/** The notes that credit any of `names`: written (author), else added (added_by), else edited (changes). */
export function creditsOf(files: Entry[], names: string[]): Credits {
  const mine = new Set(names.map(key));
  const has = (list: string[]) => list.some((n) => mine.has(key(n)));
  const out: Credits = { wrote: [], added: [], edited: [] };
  for (const f of files) {
    if (f.kind !== 'note') continue;
    const fm = f.frontmatter;
    if (has(authorsOf(fm?.author))) out.wrote.push(f);
    else if (typeof fm?.added_by === 'string' && has([fm.added_by])) out.added.push(f);
    else if (has(editorsOf(fm))) out.edited.push(f);
  }
  return out;
}

/** Everyone in the book: people who sign in (by name), then every other name a recipe credits (most recipes first). */
export function directory(files: Entry[], people: Person[]): { family: Listed[]; others: Listed[] } {
  const counts = new Map<string, { name: string; files: Set<string> }>();
  for (const f of files) {
    if (f.kind !== 'note') continue;
    const fm = f.frontmatter;
    const names = [...authorsOf(fm?.author), ...(typeof fm?.added_by === 'string' ? [fm.added_by] : []), ...editorsOf(fm)];
    for (const n of names) {
      const k = key(n);
      if (!k) continue;
      const c = counts.get(k) ?? { name: n.trim(), files: new Set() };
      c.files.add(f.path);
      counts.set(k, c);
    }
  }
  const family = people.map((p) => {
    const recipes = new Set<string>();
    for (const n of namesOf(p)) {
      const c = counts.get(key(n));
      c?.files.forEach((x) => recipes.add(x));
      counts.delete(key(n));
    }
    return { name: nameOnRecipes(p), person: p, recipes: recipes.size };
  });
  const others = [...counts.values()].map((c) => ({ name: c.name, recipes: c.files.size }))
    .sort((a, b) => b.recipes - a.recipes || a.name.localeCompare(b.name));
  return { family, others };
}

/** The person whose page `name` is, if they sign in. */
export const findPerson = (people: Person[], name: string) => people.find((p) => namesOf(p).some((n) => key(n) === key(name)));

/** Up to two initials for a picture-less avatar ("Kimberley Romo" → "KR"). */
export const initials = (name: string) => name.trim().split(/\s+/).filter((w) => /\p{L}/u.test(w)).map((w) => [...w][0]).slice(0, 2).join('').toUpperCase();
