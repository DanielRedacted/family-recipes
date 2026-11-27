<script lang="ts">
  import { onMount, getContext } from 'svelte';
  import type { Entry } from '../lib/types';
  import { groupLines, boldQty, inVersion, toCelsius, versionsOf, type Units } from '../lib/scale';
  import { renderInline, autoLink, weigh } from '../lib/markdown';
  import MadeToday from './MadeToday.svelte';
  import { app, type Tab } from '../lib/state.svelte';
  import { modal } from '../lib/modal';

  let { entry, factor, units, portion = 0, version = 0, onclose }: { entry: Entry; factor: number; units: Units; portion?: number; version?: number; onclose: () => void } = $props();
  // a recipe with versions: only the chosen one's ingredients and steps
  const v = $derived(versionsOf(entry)[version]);
  const r = $derived.by(() => {
    const all = entry.recipe!;
    const steps = all.steps.map((g) => ({ ...g, items: g.items.filter((it) => inVersion(it.line, v)) })).filter((g) => g.items.length);
    return { ...all, ingredients: all.ingredients.filter((g) => inVersion(g.items[0]?.line ?? 0, v)), steps };
  });
  const temps = (t: string) => (units === 'metric' ? toCelsius(t) : t);
  let done = $state<Record<string, boolean>>({});
  // a recipe of steps only has no Ingredients column and no tabs; its steps always show
  const hasIng = $derived(r.ingredients.length > 0);
  let tab = $state<'ing' | 'steps'>('ing');
  let awake = $state(false);
  let lock: any = null;
  // a modal dialog (Esc closes it); hidden, not ended, while another tab shows
  const page = getContext<Tab>('tab');

  onMount(() => {
    const nav: any = navigator;
    nav.wakeLock?.request('screen').then((l: any) => { lock = l; awake = true; }).catch(() => {});
    return () => lock?.release?.();
  });

  const strip = (t: string) => t.replace(/^\*\*Step \d+\*\*\s*/, '');
  const nextStep = $derived(r.steps.flatMap((g) => g.items).find((s) => !done['s' + s.line])?.line);
</script>

<dialog class="cook" aria-label="Cook mode" use:modal={{ onclose, active: page.id === app.tab }}>
  <header class="row">
    <h1>{entry.title}</h1>
    {#if v}<span class="pill">{v.label}</span>{/if}
    <span class="spacer"></span>
    {#if factor !== 1}<span class="pill">×{+factor.toFixed(2)}</span>{/if}
    <span class="muted small">{awake ? 'Screen stays on' : ''}</span>
    <button class="btn" onclick={() => (done = {})}>Reset</button>
    <button class="btn primary" onclick={onclose}>Done</button>
  </header>
  {#if hasIng}
    <div class="tabs row">
      <button class="btn" class:on={tab === 'ing'} onclick={() => (tab = 'ing')}>Ingredients</button>
      <button class="btn" class:on={tab === 'steps'} onclick={() => (tab = 'steps')}>Steps</button>
    </div>
  {/if}
  <div class="cols" class:two={hasIng}>
    {#if hasIng}
    <section class="ing" class:hide={tab !== 'ing'}>
      <h2>Ingredients</h2>
      {#each r.ingredients as g}
        {#if g.name}<h3>{g.name}</h3>{/if}
        <ul>
          {#each groupLines(g, portion, factor, units, weigh) as it (it.line)}
            {@const id = 'i' + it.line}
            <li class:done={done[id]}>
              <label><input type="checkbox" bind:checked={done[id]} /> <span>{@html renderInline(boldQty(autoLink(it.text, entry.path)))}</span></label>
            </li>
          {/each}
        </ul>
      {/each}
    </section>
    {/if}
    <section class="steps" class:hide={hasIng && tab !== 'steps'}>
      <h2>Steps</h2>
      {#each r.steps as g}
        {#if g.name}<h3>{g.name}</h3>{/if}
        <ol>
          {#each g.items as it}
            {@const id = 's' + it.line}
            <li class:done={done[id]} class:current={it.line === nextStep}>
              <label><input type="checkbox" bind:checked={done[id]} /> <span>{@html renderInline(temps(strip(it.text)))}</span></label>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="muted">No steps found in this recipe.</p>
      {/each}
    </section>
  </div>
  <MadeToday {entry} />
</dialog>

<style>
  .cook { position: fixed; inset: 0; width: auto; height: auto; max-width: none; max-height: none; margin: 0; border: 0; color: var(--text); background: var(--bg); overflow-y: auto; padding: calc(12px + env(safe-area-inset-top)) max(24px, env(safe-area-inset-right)) 40px max(24px, env(safe-area-inset-left)); font-family: var(--serif); }
  header { position: sticky; top: 0; background: var(--bg); padding-bottom: 12px; border-bottom: 1px solid var(--border); }
  h1 { margin: 0; font-size: 1.7rem; }
  .pill { background: var(--accent-soft); color: var(--accent-ink); padding: 2px 12px; border-radius: 12px; font-family: var(--sans); }
  /* steps only: one column; with an Ingredients column beside the steps: two (the class for Safari 15.0-15.3, no :has()) */
  .cols { display: grid; grid-template-columns: 1fr; gap: 40px; margin-top: 16px; }
  .cols:has(> .ing) { grid-template-columns: minmax(260px, 1fr) 2fr; }
  @supports not selector(:has(*)) { .cols.two { grid-template-columns: minmax(260px, 1fr) 2fr; } }
  h2 { font-size: 1.2rem; color: var(--muted); font-family: var(--sans); font-weight: 500; }
  h3 { color: var(--accent-ink); margin: 20px 0 8px; }
  ul, ol { list-style: none; padding: 0; margin: 0; }
  li { font-size: 22px; line-height: 1.45; padding: 8px; border-radius: 8px; }
  li label { display: flex; gap: 12px; align-items: flex-start; cursor: pointer; }
  li input { width: 24px; height: 24px; margin-top: 4px; accent-color: var(--accent); flex: none; }
  li.done span { opacity: 0.4; text-decoration: line-through; }
  li.current { background: var(--accent-soft); }
  .steps li { font-size: 24px; }
  .tabs { display: none; margin-top: 12px; }
  @media (max-width: 899px) {
    .cook { padding: calc(12px + env(safe-area-inset-top)) max(16px, env(safe-area-inset-right)) 40px max(16px, env(safe-area-inset-left)); }
    .cols, .cols:has(> .ing) { grid-template-columns: 1fr; gap: 0; }
    @supports not selector(:has(*)) { .cols.two { grid-template-columns: 1fr; } }
    .tabs { display: flex; }
    .hide { display: none; }
    h1 { font-size: 1.3rem; }
    li { font-size: 20px; } .steps li { font-size: 21px; }
  }
</style>
