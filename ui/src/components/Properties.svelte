<script lang="ts" module>
  import { Document, parseDocument } from 'yaml';

  export const LABELS: Record<string, string> = {
    tags: 'Tags', yield: 'Yield', servings: 'Servings', prep_time: 'Prep', cook_time: 'Cook', total_time: 'Total', additional_time: 'Additional',
    source: 'Source', author: 'Author', original: 'Original file', needs_review: 'Needs review', created: 'Created',
    emoji: 'Icon', color: 'Card colour', nutrition: 'Nutrition', favorite: 'Favorite', note: 'Note', aliases: 'Also called',
  };

  /** Split raw text into [yaml, body]; yaml is '' when there is no frontmatter. */
  export function splitRaw(raw: string): [string, string] {
    const m = raw.match(/^---\r?\n([\s\S]*?\r?\n)?---[ \t]*(?:\r?\n|$)/);
    return m ? [m[1] ?? '', raw.slice(m[0].length)] : ['', raw];
  }

  /** Set (or with value===undefined delete) one frontmatter key, keeping comments and order. */
  export function setProp(raw: string, key: string, value: unknown): string {
    const [yaml, body] = splitRaw(raw);
    // a new block-style map for a note without properties ("favorite: true", not "{ favorite: true }")
    const doc = yaml.trim() ? parseDocument(yaml) : new Document({});
    if (value === undefined || value === '' || (Array.isArray(value) && !value.length)) doc.delete(key);
    else doc.set(key, value);
    const out = doc.contents && (doc.contents as any).items?.length ? String(doc) : '';
    const fm = out && out.trim() !== '{}' ? `---\n${out.endsWith('\n') ? out : out + '\n'}---\n` : '';
    return fm + (fm && body && !body.startsWith('\n') ? '\n' : '') + body;
  }
</script>

<script lang="ts">
  import type { Entry } from '../lib/types';
  import { tagsOf } from '../lib/search';
  import { vaultTarget } from '../lib/markdown';
  import { noteHash } from '../lib/state.svelte';
  import Icon from './Icon.svelte';

  /** `collapsible`: inside a recipe's card, where the add (+) pills, source, tags and original file wait behind More */
  let { entry, editable, onchange, collapsible = false }: { entry: Entry; editable: boolean; onchange: (raw: string) => void; collapsible?: boolean } = $props();
  let open = $state(false);
  /** a missing standard property being filled in from its chip */
  let adding = $state<string | null>(null);
  // a recipe always shows these, with an Add button when they are missing
  const STANDARD = ['yield', 'servings', 'prep_time', 'cook_time', 'total_time', 'source'];
  const missing = $derived(editable && entry.recipe ? STANDARD.filter((k) => !(k in fm)) : []);

  function addValue(k: string, text: string) {
    if (adding !== k) return; // Enter already saved it (the field's blur follows)
    adding = null;
    if (text.trim()) update(k, text);
  }
  const focus = (el: HTMLInputElement) => el.focus();

  const fm = $derived(entry.frontmatter ?? {});
  const keys = $derived(Object.keys(fm));
  // shown elsewhere: tags as chips, nutrition as its own table, icon/colour on the cards
  const HIDDEN = ['tags', 'needs_review', 'original', 'nutrition', 'emoji', 'color', 'favorite', 'author'];
  const fmtVal = (v: unknown) => (Array.isArray(v) ? v.join(', ') : v === true ? 'yes' : v === false ? 'no' : v == null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v));

  function update(key: string, text: string) {
    let v: unknown = text.trim();
    if (key === 'tags' || key === 'aliases') v = text.split(',').map((s) => s.trim()).filter(Boolean);
    else if (key === 'needs_review' || key === 'favorite') v = text.trim() && !/^(no|false)$/i.test(text.trim()) ? true : undefined;
    else if (/^\d+(\.\d+)?$/.test(text.trim())) v = Number(text.trim());
    onchange(setProp(entry.raw ?? '', key, v));
  }

  // in a recipe's card the details show at once; the rest waits behind More
  const shown = $derived(keys.filter((k) => !HIDDEN.includes(k) && !(collapsible && k === 'source')));
  const orig = $derived(typeof fm.original === 'string' ? vaultTarget(fm.original, '') : null);
  const hasMore = $derived(!!fm.source || missing.length > 0 || tagsOf(entry).length > 0 || !!orig);
</script>

{#snippet prop(k: string)}
  <span class="chip"><span class="k">{LABELS[k] ?? k}</span>
    {#if k === 'source' && /^https?:/.test(String(fm[k]))}
      <a class="ext" href={String(fm[k])} target="_blank" rel="noopener noreferrer">{new URL(String(fm[k])).hostname}</a>
    {:else}{fmtVal(fm[k])}{/if}
  </span>
{/snippet}

{#if keys.length || editable}
  <section class="props" aria-label="Properties">
    <div class="chips">
        {#if fm.favorite === true}<span class="chip fav">★ Favorite</span>{/if}
        {#each shown as k}{@render prop(k)}{/each}
        {#if collapsible && hasMore}
          <button class="chip toggle" aria-expanded={open} onclick={() => (open = !open)}>{open ? 'Less' : 'More'}<Icon name={open ? 'chevron-up' : 'chevron-down'} size={14} /></button>
        {/if}
      </div>
      {#if !collapsible || open}
      <div class="chips more">
        {#if collapsible && fm.source}{@render prop('source')}{/if}
        {#each missing as k (k)}
          {#if adding === k}
            <span class="chip adding"><span class="k">{LABELS[k]}</span>
              <input class="inline" use:focus aria-label={LABELS[k]} placeholder={k === 'source' ? 'https://…' : k.endsWith('_time') ? 'e.g. 20 min' : 'e.g. 12'}
                onkeydown={(e) => { if (e.key === 'Enter') addValue(k, e.currentTarget.value); else if (e.key === 'Escape') adding = null; }}
                onblur={(e) => addValue(k, e.currentTarget.value)} />
            </span>
          {:else}
            <button class="chip missing" aria-label="Add {LABELS[k]}" onclick={() => (adding = k)}><span class="k">{LABELS[k]}</span><Icon name="plus" size={14} /></button>
          {/if}
        {/each}
        {#each tagsOf(entry) as t}<span class="chip tag">#{t}</span>{/each}
        {#if orig}<a class="chip" href={noteHash(orig)}><Icon name="paperclip" size={14} /> Original file</a>{/if}
      </div>
      {/if}
    {#if entry.frontmatter_error}<p class="warn small">Properties block could not be read: {entry.frontmatter_error}</p>{/if}
  </section>
{/if}

<style>
  .props { margin: 0 0 16px; font-size: 14px; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .chip.toggle { border: 0; cursor: pointer; font: inherit; color: var(--accent-ink); background: none; padding: 4px 8px; }
  .chip { display: inline-flex; align-items: center; gap: 4px; background: var(--sunken); border-radius: 16px; padding: 4px 12px; text-decoration: none; color: var(--text); }
  .chip .k { color: var(--muted); }
  .chip.tag { color: var(--accent-ink); background: var(--accent-soft); }
  .chip.fav { background: var(--p-yellow); }
  .chip.missing { border: 1px dashed var(--border); background: none; color: var(--muted); cursor: pointer; font: inherit; }
  .chip.missing:hover { border-color: var(--accent); color: var(--accent-ink); }
  .chip.adding { display: inline-flex; align-items: center; }
  .inline { border: 0; background: var(--surface); border-radius: 8px; padding: 2px 8px; width: 9em; font: inherit; }
  .inline:focus { outline: 2px solid var(--accent); }
  .warn { color: var(--danger); }
</style>
