<script lang="ts">
  import Back from './Back.svelte';
  import { onMount } from 'svelte';
  import { app, noteHash, dirOf, toast } from '../lib/state.svelte';
  import { catalog, loadIngredients, foods, loadFoods } from '../lib/ingredients.svelte';
  import { api } from '../lib/api';
  import { ingredientIcon } from '../lib/icons';
  import CardIcon from './CardIcon.svelte';

  let { query = '' }: { query?: string } = $props();
  let q = $state('');
  let open = $state<string | null>(null);
  let sort = $state<'name' | 'count'>('count');
  $effect(() => { q = query; open = null; });
  onMount(() => { loadIngredients(0); loadFoods(true); });

  // which USDA food an ingredient counts as in nutrition; the admin can choose another or leave it out
  const admin = $derived(app.session.role === 'admin');
  let picking = $state<string | null>(null);
  let pq = $state('');
  let hits = $state<{ fdc: number; name: string; kind: string }[] | null>(null);
  let busy = $state(false);
  async function run(f: () => Promise<unknown>, ok?: string) {
    busy = true;
    try { await f(); if (ok) toast(ok, 'ok'); } catch (e: any) { toast(e.message, 'error'); }
    busy = false;
  }
  const find = () => run(async () => { hits = await api('GET', '/api/foods/search?q=' + encodeURIComponent(pq.trim())); });
  const choose = (key: string, fdc: number | null) => run(async () => {
    await api('PUT', `/api/foods/${encodeURIComponent(key)}`, { fdc });
    await loadFoods(true);
    picking = null; hits = null;
  }, fdc ? 'Nutrition for this ingredient changed' : 'Left out of nutrition');
  const undo = (key: string) => run(async () => { await api('DELETE', `/api/foods/${encodeURIComponent(key)}`); await loadFoods(true); }, 'Choice undone');

  const rows = $derived.by(() => {
    const t = q.trim().toLowerCase();
    const hit = t ? catalog.items.filter((i) => i.name.toLowerCase().includes(t) || i.forms.some((f) => f.toLowerCase().includes(t))) : catalog.items;
    return sort === 'count' ? [...hit].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name)) : hit;
  });
  $effect(() => { if (rows.length === 1) open = rows[0].key; });
  const title = (p: string) => app.byPath[p]?.title ?? p;
</script>

<div class="page">
  <h1 class="title"><Back />Ingredients</h1>
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
            <span class="name"><span class="ic"><CardIcon icon={ingredientIcon(i.name)} size={17} /></span>{i.name}</span>
            <span class="muted small">{i.count} recipe{i.count === 1 ? '' : 's'}</span>
          </button>
          {#if open === i.key}
            <div class="more">
              {#if i.forms.length}<p class="muted small">Also written as: {i.forms.join(' · ')}</p>{/if}
              <ul>{#each i.recipes as p}<li><a href={noteHash(p)}>{title(p)}</a> <span class="muted small">{dirOf(p)}</span></li>{/each}</ul>
              {#if foods.loaded}
                {@const food = foods.table[i.key]}
                <div class="food small">
                  {#if food}<span>Nutrition: counted as <b>{food.name}</b> (USDA){#if food.auto} <span class="muted">— found automatically, worth a check</span>{/if}</span>
                  {:else if food === null}<span>Nutrition: left out</span>
                  {:else}<span class="muted">Nutrition: not in the list yet, so recipes don't count it</span>{/if}
                  {#if admin}
                    <button class="btn ghost small" onclick={() => { picking = picking === i.key ? null : i.key; pq = i.name; hits = null; }} aria-expanded={picking === i.key}>Change…</button>
                    {#if food !== null}<button class="btn ghost small" disabled={busy} onclick={() => choose(i.key, null)}>Don't count</button>{/if}
                    {#if foods.own.includes(i.key)}<button class="btn ghost small" disabled={busy} onclick={() => undo(i.key)}>Undo</button>{/if}
                  {/if}
                </div>
                {#if picking === i.key}
                  <form class="row pick" onsubmit={(e) => { e.preventDefault(); find(); }}>
                    <input class="field grow" bind:value={pq} aria-label="Search USDA foods" enterkeyhint="search" />
                    <button class="btn" disabled={busy || !pq.trim()}>Search USDA</button>
                  </form>
                  {#if hits}
                    <ul class="hits">
                      {#each hits as h (h.fdc)}<li><button class="btn ghost small" disabled={busy} onclick={() => choose(i.key, h.fdc)}>{h.name}</button> <span class="muted small">{h.kind}</span></li>{/each}
                      {#if !hits.length}<li class="muted">No USDA foods match. Try fewer or other words.</li>{/if}
                    </ul>
                  {/if}
                {/if}
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .title { display: flex; align-items: center; gap: 4px; margin-left: -8px; }
  .page { padding: 16px 28px 80px; max-width: 820px; }
  h1 { margin: 8px 0 4px; }
  .bar { margin: 12px 0 8px; }
  .grow { flex: 1; min-width: 200px; }
  .w { width: auto; }
  .list { list-style: none; padding: 0; margin: 0; }
  .list > li { border-bottom: 1px solid var(--border); }
  .head { width: 100%; display: flex; justify-content: space-between; gap: 12px; padding: 8px 4px; background: none; border: 0; cursor: pointer; text-align: left; color: var(--text); }
  .head:hover { background: var(--sunken); }
  .name { font-family: var(--serif); font-size: 16px; display: flex; align-items: center; gap: 8px; }
  .ic { display: inline-flex; width: 20px; justify-content: center; flex: none; }
  .more { padding: 0 8px 12px; }
  .more p { margin: 0 0 8px; }
  .more ul { margin: 0; padding-left: 20px; columns: 2 240px; }
  .more li { break-inside: avoid; margin: 2px 0; }
  .food { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 8px; margin-top: 10px; }
  .pick { margin-top: 8px; gap: 8px; }
  .more ul.hits { columns: auto; list-style: none; padding: 0; margin-top: 4px; }
  .hits .btn { text-align: start; white-space: normal; }
  @media (max-width: 899px) { .page { padding: 8px 16px 96px; } }
</style>
