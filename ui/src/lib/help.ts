// The in-app Help pages: Markdown files in docs/help/ (plus docs/FORMAT.md), bundled at build time.
// Admin pages are hidden from other people, not secret: their text ships in the bundle like the rest.
import MiniSearch from 'minisearch';
import { parse } from 'yaml';
import { splitRaw } from './frontmatter';
import format from '../../../docs/FORMAT.md?raw';

export interface HelpPage { slug: string; title: string; summary: string; audience: 'everyone' | 'admin'; order: number; body: string }

const files = import.meta.glob('../../../docs/help/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

export const pages: HelpPage[] = [
  ...Object.entries(files).map(([file, raw]) => {
    const [yaml, body] = splitRaw(raw);
    const fm = (parse(yaml) ?? {}) as Partial<HelpPage>;
    return { slug: file.split('/').pop()!.replace(/\.md$/, ''), title: fm.title ?? '', summary: fm.summary ?? '', audience: fm.audience ?? 'everyone', order: fm.order ?? 999, body };
  }),
  // FORMAT.md stays where agents and GitHub expect it; its own "# Title" heading is dropped here
  { slug: 'file-format', title: 'Recipe file format', summary: 'How a recipe file is laid out and how the app reads it.', audience: 'admin' as const, order: 75, body: format.replace(/^# .*\n+/, '') },
].sort((a, b) => a.order - b.order || a.title.localeCompare(b.title));

/** The pages one person can see, in reading order. */
export const helpPages = (admin: boolean) => pages.filter((p) => admin || p.audience === 'everyone');

let index: MiniSearch<HelpPage> | null = null;

/** Pages matching `q`, best first, limited to what this person can see. */
export function searchHelp(q: string, admin: boolean): HelpPage[] {
  if (!index) {
    index = new MiniSearch<HelpPage>({ idField: 'slug', fields: ['title', 'body'], searchOptions: { boost: { title: 3 }, prefix: true, fuzzy: 0.2, combineWith: 'AND' } });
    index.addAll(pages);
  }
  const bySlug = new Map(helpPages(admin).map((p) => [p.slug, p]));
  return index.search(q).map((r) => bySlug.get(r.id as string)).filter((p): p is HelpPage => !!p);
}
