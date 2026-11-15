import MiniSearch from 'minisearch';
import type { Entry } from './types';

// rebuilt from scratch on every vault change; fine to a few thousand notes.
let index: MiniSearch | null = null;

export function buildIndex(files: Entry[]) {
  index = new MiniSearch({
    fields: ['title', 'name', 'aliases', 'folder', 'tags', 'ingredients', 'text'],
    storeFields: ['title', 'path', 'fav'],
    idField: 'path',
    searchOptions: {
      boost: { title: 4, name: 3, aliases: 3, tags: 2, ingredients: 2, folder: 1.5 }, prefix: true, fuzzy: 0.2, combineWith: 'AND',
      boostDocument: (_id, _term, stored) => (stored?.fav ? 1.5 : 1), // favorites first among similar matches
    },
  });
  index.addAll(files.filter((f) => f.kind !== 'other' || true).map((f) => ({
    path: f.path,
    title: f.title,
    name: f.path.split('/').pop(),
    aliases: aliasesOf(f).join(' '),
    fav: f.frontmatter?.favorite === true,
    folder: f.path.split('/').slice(0, -1).join(' '),
    tags: tagsOf(f).join(' '),
    ingredients: f.recipe?.ingredients.flatMap((g) => g.items.map((i) => i.text)).join(' ') ?? '',
    text: f.raw ?? f.text ?? '',
  })));
}

export function search(q: string, limit = 50): { path: string; title: string }[] {
  if (!index || !q.trim()) return [];
  return index.search(q).slice(0, limit).map((r) => ({ path: r.id as string, title: r.title as string }));
}

/** Other names a recipe is known by (`aliases:` in its properties). */
export function aliasesOf(f: Entry): string[] {
  const a = f.frontmatter?.aliases;
  return Array.isArray(a) ? a.map(String) : typeof a === 'string' && a.trim() ? [a.trim()] : [];
}

export function tagsOf(f: Entry): string[] {
  const t = f.frontmatter?.tags;
  if (Array.isArray(t)) return t.map(String);
  if (typeof t === 'string') return t.split(',').map((s) => s.trim()).filter(Boolean);
  return [];
}
