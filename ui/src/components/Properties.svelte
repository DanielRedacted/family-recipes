<script lang="ts" module>
  import { parseDocument } from 'yaml';

  export const KNOWN = ['tags', 'yield', 'servings', 'prep_time', 'cook_time', 'total_time', 'source', 'author', 'original', 'needs_review'];
  export const LABELS: Record<string, string> = {
    tags: 'Tags', yield: 'Yield', servings: 'Servings', prep_time: 'Prep', cook_time: 'Cook', total_time: 'Total',
    source: 'Source', author: 'Author', original: 'Original file', needs_review: 'Needs review', created: 'Created',
  };

  /** Split raw text into [yaml, body]; yaml is '' when there is no frontmatter. */
  export function splitRaw(raw: string): [string, string] {
    const m = raw.match(/^---\r?\n([\s\S]*?\r?\n)?---[ \t]*(?:\r?\n|$)/);
    return m ? [m[1] ?? '', raw.slice(m[0].length)] : ['', raw];
  }

  /** Set (or with value===undefined delete) one frontmatter key, keeping comments and order. */
  export function setProp(raw: string, key: string, value: unknown): string {
    const [yaml, body] = splitRaw(raw);
    const doc = parseDocument(yaml || '{}');
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

  let { entry, editable, onchange }: { entry: Entry; editable: boolean; onchange: (raw: string) => void } = $props();
  let editing = $state(false);
  let newKey = $state('');

  const fm = $derived(entry.frontmatter ?? {});
  const keys = $derived(Object.keys(fm));
  const fmtVal = (v: unknown) => (Array.isArray(v) ? v.join(', ') : v === true ? 'yes' : v === false ? 'no' : v == null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v));

  function update(key: string, text: string) {
    let v: unknown = text.trim();
    if (key === 'tags') v = text.split(',').map((s) => s.trim()).filter(Boolean);
    else if (key === 'needs_review') v = text.trim() ? true : undefined;
    else if (/^\d+(\.\d+)?$/.test(text.trim())) v = Number(text.trim());
    onchange(setProp(entry.raw ?? '', key, v));
  }

  function add() {
    const k = newKey.trim().toLowerCase().replace(/\s+/g, '_');
    if (!k || k in fm) return;
    onchange(setProp(entry.raw ?? '', k, k === 'tags' ? ['recipe'] : 'TBD'));
    newKey = '';
  }

  const orig = $derived(typeof fm.original === 'string' ? vaultTarget(fm.original, '') : null);
</script>

{#if keys.length || editable}
  <section class="props" aria-label="Properties">
    {#if !editing}
      <div class="chips">
        {#each keys.filter((k) => k !== 'tags' && k !== 'needs_review' && k !== 'original') as k}
          <span class="chip"><span class="k">{LABELS[k] ?? k}</span>
            {#if k === 'source' && /^https?:/.test(String(fm[k]))}
              <a href={String(fm[k])} target="_blank" rel="noopener noreferrer">{new URL(String(fm[k])).hostname}</a>
            {:else}{fmtVal(fm[k])}{/if}
          </span>
        {/each}
        {#each tagsOf(entry) as t}<span class="chip tag">#{t}</span>{/each}
        {#if orig}<a class="chip" href={noteHash(orig)}>📎 Original file</a>{/if}
        {#if editable}<button class="btn ghost small" onclick={() => (editing = true)}>{keys.length ? 'Edit properties' : '＋ Add properties'}</button>{/if}
      </div>
      {#if entry.frontmatter_error}<p class="warn small">Properties block could not be read: {entry.frontmatter_error}</p>{/if}
    {:else}
      <table>
        <tbody>
          {#each keys as k (k)}
            <tr>
              <th>{LABELS[k] ?? k}</th>
              <td><input class="field" value={fmtVal(fm[k])} onchange={(e) => update(k, e.currentTarget.value)} aria-label={LABELS[k] ?? k} /></td>
              <td><button class="btn ghost icon" aria-label="Remove {k}" onclick={() => update(k, '')}>✕</button></td>
            </tr>
          {/each}
          <tr>
            <th>
              <input class="field" list="known-props" placeholder="Add property" bind:value={newKey} onkeydown={(e) => e.key === 'Enter' && add()} aria-label="New property name" />
              <datalist id="known-props">{#each KNOWN.filter((k) => !(k in fm)) as k}<option value={k}>{LABELS[k]}</option>{/each}</datalist>
            </th>
            <td colspan="2"><button class="btn" onclick={add} disabled={!newKey.trim()}>Add</button></td>
          </tr>
        </tbody>
      </table>
      <button class="btn small" onclick={() => (editing = false)}>Done</button>
    {/if}
  </section>
{/if}

<style>
  .props { margin: 0 0 18px; font-size: 14px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .chip { background: var(--sunken); border-radius: 14px; padding: 3px 10px; text-decoration: none; color: var(--text); }
  .chip .k { color: var(--muted); margin-right: 5px; }
  .chip.tag { color: var(--accent-ink); background: var(--accent-soft); }
  table { border-collapse: collapse; width: 100%; max-width: 640px; margin-bottom: 8px; }
  th { text-align: left; font-weight: 500; color: var(--muted); padding: 3px 8px 3px 0; width: 140px; }
  td { padding: 3px 0; }
  .warn { color: var(--danger); }
</style>
