// Editor suggestions: [[links]] to other recipes.
import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import { app, stemOf } from './state.svelte';

/** `[[` → note titles (inserted as `[[Title]]`). */
export function wikiLinks(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/\[\[[^\]\n]*/);
  if (!m) return null;
  const closed = ctx.state.sliceDoc(ctx.pos, ctx.pos + 2) === ']]';
  const seen = new Set<string>();
  const options = app.files
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
