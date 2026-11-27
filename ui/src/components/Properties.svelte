<script lang="ts" module>
  export const LABELS: Record<string, string> = {
    tags: 'Tags', yield: 'Yield', servings: 'Servings', prep_time: 'Prep', cook_time: 'Cook', total_time: 'Total', additional_time: 'Additional',
    source: 'Source', author: 'Author', original: 'Original file', needs_review: 'Needs review', created: 'Created',
    emoji: 'Icon', color: 'Card colour', nutrition: 'Nutrition', favorite: 'Bookmarked', note: 'Note', aliases: 'Also called',
    added_by: 'Added by', changes: 'History',
  };
  /** not in the row of details: tags as chips, nutrition as its own table, icon/colour on the cards;
   *  added_by and changes under More, with the source */
  export const HIDDEN = ['tags', 'needs_review', 'original', 'nutrition', 'emoji', 'color', 'favorite', 'author', 'added_by', 'changes', 'versions'];
  export const fmtVal = (v: unknown) => (Array.isArray(v) ? v.join(', ') : v === true ? 'yes' : v === false ? 'no' : v == null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v));
</script>

<script lang="ts">
  import type { Entry } from '../lib/types';
  import { setProp } from '../lib/frontmatter';
  import { tagsOf } from '../lib/search';
  import { vaultTarget } from '../lib/markdown';
  import { noteHash } from '../lib/state.svelte';
  import { personHash } from '../lib/people';
  import Icon from './Icon.svelte';

  /** `collapsible`: inside a recipe's card, where the add (+) pills, source, tags and original file wait behind More */
  /** `onchange` gets the new text and the property that changed */
  let { entry, editable, onchange, collapsible = false }: { entry: Entry; editable: boolean; onchange: (raw: string, key: string) => void; collapsible?: boolean } = $props();
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

  function update(key: string, text: string) {
    let v: unknown = text.trim();
    if (key === 'tags' || key === 'aliases') v = text.split(',').map((s) => s.trim()).filter(Boolean);
    else if (key === 'needs_review' || key === 'favorite') v = text.trim() && !/^(no|false)$/i.test(text.trim()) ? true : undefined;
    else if (/^\d+(\.\d+)?$/.test(text.trim())) v = Number(text.trim());
    onchange(setProp(entry.raw ?? '', key, v), key);
  }

  // in a recipe's card the details show at once; the rest waits behind More
  const shown = $derived(keys.filter((k) => !HIDDEN.includes(k) && !(collapsible && k === 'source')));
  const orig = $derived(typeof fm.original === 'string' ? vaultTarget(fm.original, '') : null);
  /** the recipe's change history, newest first (lib/credit.ts) */
  const history = $derived((Array.isArray(fm.changes) ? fm.changes : []).filter((c: any) => c && typeof c === 'object').slice().reverse() as { by?: string; date?: string; note?: string }[]);
  const addedBy = $derived(typeof fm.added_by === 'string' ? fm.added_by : '');
  const hasMore = $derived(!!fm.source || !!addedBy || history.length > 0 || missing.length > 0 || tagsOf(entry).length > 0 || !!orig);
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
        {#if fm.favorite === true}<span class="chip fav"><Icon name="bookmark" size={14} /> Bookmarked</span>{/if}
        {#each shown as k}{@render prop(k)}{/each}
        {#if collapsible && hasMore}
          <button class="chip toggle" aria-expanded={open} onclick={() => (open = !open)}>{open ? 'Less' : 'More'}<Icon name={open ? 'chevron-up' : 'chevron-down'} size={14} /></button>
        {/if}
      </div>
      {#if !collapsible || open}
      <div class="chips more">
        {#if collapsible && fm.source}{@render prop('source')}{/if}
        {#if addedBy}<a class="chip" href={personHash(addedBy)}><span class="k">Added by</span>{addedBy}</a>{/if}
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
      {#if history.length}
        <p class="muted small hhead">History</p>
        <ol class="history" aria-label="History">
          {#each history as c}<li><span class="muted">{c.date ?? ''}</span> {#if c.by}<a class="who" href={personHash(c.by)}>{c.by}</a>{#if c.note} · {/if}{/if}{c.note ?? ''}</li>{/each}
        </ol>
      {/if}
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
  .hhead { margin: 12px 0 4px; }
  .who { color: inherit; text-decoration-color: var(--border); text-underline-offset: 2px; }
  .who:hover { color: var(--accent-ink); text-decoration-color: currentColor; }
  .history { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; max-height: 200px; overflow-y: auto; }
</style>
