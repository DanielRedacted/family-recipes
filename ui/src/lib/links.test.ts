import { describe, expect, it, vi } from 'vitest';
import type { Entry } from './types';

const note = (path: string, title: string, author?: string): Entry =>
  ({ path, title, kind: 'note', size: 0, mtime: 0, etag: '', body_line: 0, frontmatter: author ? { author } : {} });
const files = [note('Pies/Pie Crust.md', 'Pie Crust', 'Test Cook'), note('Soups/Stew.md', 'Stew', 'Dee Ray')];
const app = { files, people: [{ id: 1, name: 'Ann', full_name: 'Ann Lee' }] };
vi.mock('./state.svelte', () => ({
  app,
  noteHash: (p: string) => '#/n/' + encodeURI(p),
  resolveLink: (t: string) => app.files.find((f) => f.title.toLowerCase() === t.toLowerCase()),
  dirOf: () => '',
  stemOf: (p: string) => p,
}));
vi.mock('dompurify', () => ({ default: { sanitize: (s: string) => s } })); // no DOM here; the markup is what's tested
vi.mock('./api', () => ({ fileUrl: (p: string) => p, api: () => new Promise(() => {}) }));
vi.mock('./ingredients.svelte', () => ({
  foods: { table: {} },
  catalog: { loaded: true, items: [{ name: 'butter', key: 'butter', count: 3, recipes: [], forms: [] }, { name: 'brown sugar', key: 'brown sugar', count: 2, recipes: [], forms: ['light brown sugar'] }] },
  loadIngredients: () => Promise.resolve(),
}));
const { renderInline } = await import('./markdown');

describe('links to recipes, people and ingredients', () => {
  it('links @people and #ingredients by the longest known name', () => {
    const out = renderInline('Ask @Ann Lee or @test cook about #brown sugar, then #butter.');
    expect(out).toContain('<a class="wikilink mention" href="#/people/Ann%20Lee">Ann Lee</a> or');
    expect(out).toContain('href="#/people/Test%20Cook">test cook</a>');
    expect(out).toContain('href="#/ingredients/brown%20sugar">brown sugar</a>,');
    expect(out).toContain('href="#/ingredients/butter">butter</a>.');
  });

  it('links a short or former name to the person’s page, and another spelling to the ingredient', () => {
    expect(renderInline('@ann')).toContain('href="#/people/Ann%20Lee">ann</a>');
    expect(renderInline('#light brown sugar')).toContain('href="#/ingredients/brown%20sugar"');
  });

  it('leaves everything else as text', () => {
    expect(renderInline('[use #butter](https://example.com)')).not.toContain('mention');
    for (const s of ['a #10 can', 'Step #2', 'mail ann@example.com', 'C#butter', '@Annie', '#butters', '&#35;'])
      expect(renderInline(s)).not.toContain('mention');
  });

  it('lets [[ ]] reach a person or an ingredient when no recipe has the name', () => {
    expect(renderInline('[[Pie Crust]]')).toContain('href="#/n/Pies/Pie%20Crust.md"');
    expect(renderInline('[[Dee Ray]]')).toContain('href="#/people/Dee%20Ray"');
    expect(renderInline('[[butter|the butter]]')).toContain('href="#/ingredients/butter">the butter</a>');
    expect(renderInline('[[Nobody]]')).toContain('wikilink missing');
  });
});
