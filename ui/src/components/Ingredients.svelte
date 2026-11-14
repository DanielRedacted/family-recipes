<script lang="ts">
  import { onMount } from 'svelte';
  import { app, noteHash, dirOf } from '../lib/state.svelte';
  import { catalog, loadIngredients } from '../lib/ingredients.svelte';

  let { query = '' }: { query?: string } = $props();
  let q = $state('');
  let open = $state<string | null>(null);
  let sort = $state<'name' | 'count'>('count');
  $effect(() => { q = query; open = null; });
  onMount(() => loadIngredients(0));

  const rows = $derived.by(() => {
    const t = q.trim().toLowerCase();
    const hit = t ? catalog.items.filter((i) => i.name.toLowerCase().includes(t) || i.forms.some((f) => f.toLowerCase().includes(t))) : catalog.items;
    return sort === 'count' ? [...hit].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name)) : hit;
  });
  $effect(() => { if (rows.length === 1) open = rows[0].key; });
  const title = (p: string) => app.byPath[p]?.title ?? p;
</script>

<div class="page">
  <h1>Ingredients</h1>
  <p class="muted small">Every ingredient in your recipes. Spellings of the same thing (“Eggs”, “2 large eggs, beaten”) are grouped together.</p>
  <div class="row bar">
    <input class="field grow" placeholder="Find an ingredient…" bind:value={q} aria-label="Find an ingredient" />
    <select class="field w" bind:value={sort} aria-label="Sort">
      <option value="count">Most used</option>
      <option value="name">A–Z</option>
    </select>
  </div>
  {#if !catalog.loaded}<p class="muted">Loading…</p>
  {:else}
    <p class="muted small">{rows.length} ingredient{rows.length === 1 ? '' : 's'}</p>
    <ul class="list">
      {#each rows as i (i.key)}
        <li class:open={open === i.key}>
          <button class="head" onclick={() => (open = open === i.key ? null : i.key)} aria-expanded={open === i.key}>
            <span class="name">{i.name}</span>
            <span class="muted small">{i.count} recipe{i.count === 1 ? '' : 's'}</span>
          </button>
          {#if open === i.key}
            <div class="more">
              {#if i.forms.length}<p class="muted small">Also written as: {i.forms.join(' · ')}</p>{/if}
              <ul>{#each i.recipes as p}<li><a href={noteHash(p)}>{title(p)}</a> <span class="muted small">{dirOf(p)}</span></li>{/each}</ul>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .page { padding: 16px 28px 80px; max-width: 820px; }
  h1 { margin: 6px 0 4px; }
  .bar { margin: 12px 0 6px; }
  .grow { flex: 1; min-width: 200px; }
  .w { width: auto; }
  .list { list-style: none; padding: 0; margin: 0; }
  .list > li { border-bottom: 1px solid var(--border); }
  .head { width: 100%; display: flex; justify-content: space-between; gap: 12px; padding: 9px 4px; background: none; border: 0; cursor: pointer; text-align: left; color: var(--text); }
  .head:hover { background: var(--sunken); }
  .name { font-family: var(--serif); font-size: 16px; }
  .more { padding: 0 8px 12px; }
  .more p { margin: 0 0 6px; }
  .more ul { margin: 0; padding-left: 18px; columns: 2 240px; }
  .more li { break-inside: avoid; margin: 2px 0; }
  @media (max-width: 899px) { .page { padding: 8px 14px 96px; } }
</style>
