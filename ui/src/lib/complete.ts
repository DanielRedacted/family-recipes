// Editor suggestions: [[links]] to recipes, @people and #ingredients. `[[` offers recipes only, so a recipe is
// never outranked by a shorter ingredient name; a typed [[Name]] still reaches a person or ingredient (markdown.ts).
import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import { app, stemOf } from './state.svelte';
import { peopleNames } from './people';
import { catalog, loadIngredients } from './ingredients.svelte';

const people = (): Completion[] =>
  [...new Set(peopleNames(app.files, app.people).values())].map((label) => ({ label, detail: 'person', type: 'class', boost: -1 }));
const ingredients = (): Completion[] => {
  if (!catalog.loaded) loadIngredients();
  return catalog.items.map((i) => ({ label: i.name, detail: 'ingredient', type: 'variable', boost: Math.min(i.count, 99) - 200 }));
};

/** `@` → people, `#` → ingredients, at the start of a word (not `a@b`, `C#`, or a `# Heading`). */
export function mentions(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/[@#](?:[\p{L}\p{N}][\p{L}\p{N}' -]*)?/u);
  if (!m) return null;
  const line = ctx.state.doc.lineAt(m.from);
  const prev = ctx.state.sliceDoc(m.from - 1, m.from);
  if (m.from > line.from && /[\p{L}\p{N}_@#&]/u.test(prev)) return null;
  const at = ctx.state.sliceDoc(m.from, m.from + 1) === '@';
  if (!at && m.from === line.from) return null; // a heading being typed
  return { from: m.from + 1, options: at ? people() : ingredients(), validFor: /^[\p{L}\p{N}' -]*$/u };
}

/** `[[` → note titles (inserted as `[[Title]]`). */
export function wikiLinks(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/\[\[[^\]\n]*/);
  if (!m) return null;
  const closed = ctx.state.sliceDoc(ctx.pos, ctx.pos + 2) === ']]';
  const seen = new Set<string>();
  const options: Completion[] = app.files
    .filter((f) => f.kind === 'note')
    .flatMap((f) => {
      const stem = stemOf(f.path);
      const label = f.title || stem;
      if (seen.has(label.toLowerCase())) return [];
      seen.add(label.toLowerCase());
      const dir = f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '';
      return [{ label, detail: dir, apply: closed ? label : label + ']]', type: 'text' }];
    });
  return { from: m.from + 2, options, validFor: /^[^\]\n]*$/ };
}
