<script lang="ts">
  import { onMount } from 'svelte';
  import type { Entry } from '../lib/types';
  import { groupLines, boldQty, type Units } from '../lib/scale';
  import { renderInline, autoLink } from '../lib/markdown';

  let { entry, factor, units, portion = 0, onclose }: { entry: Entry; factor: number; units: Units; portion?: number; onclose: () => void } = $props();
  const r = $derived(entry.recipe!);
  let done = $state<Record<string, boolean>>({});
  let tab = $state<'ing' | 'steps'>('ing');
  let awake = $state(false);
  let lock: any = null;

  onMount(() => {
    const nav: any = navigator;
    nav.wakeLock?.request('screen').then((l: any) => { lock = l; awake = true; }).catch(() => {});
    const key = (e: KeyboardEvent) => e.key === 'Escape' && onclose();
    window.addEventListener('keydown', key);
    return () => { lock?.release?.(); window.removeEventListener('keydown', key); };
  });

  const strip = (t: string) => t.replace(/^\*\*Step \d+\*\*\s*/, '');
  const nextStep = $derived(r.steps.flatMap((g) => g.items).find((s) => !done['s' + s.line])?.line);
</script>

<div class="cook" role="dialog" aria-modal="true" aria-label="Cook mode">
  <header class="row">
    <h1>{entry.title}</h1>
    <span class="spacer"></span>
    {#if factor !== 1}<span class="pill">×{+factor.toFixed(2)}</span>{/if}
    <span class="muted small">{awake ? 'Screen stays on' : ''}</span>
    <button class="btn" onclick={() => (done = {})}>Reset</button>
    <button class="btn primary" onclick={onclose}>Done</button>
  </header>
  <div class="tabs row">
    <button class="btn" class:on={tab === 'ing'} onclick={() => (tab = 'ing')}>Ingredients</button>
    <button class="btn" class:on={tab === 'steps'} onclick={() => (tab = 'steps')}>Steps</button>
  </div>
  <div class="cols">
    <section class="ing" class:hide={tab !== 'ing'}>
      <h2>Ingredients</h2>
      {#each r.ingredients as g}
        {#if g.name}<h3>{g.name}</h3>{/if}
        <ul>
          {#each groupLines(g, portion, factor, units) as it (it.line)}
            {@const id = 'i' + it.line}
            <li class:done={done[id]}>
              <label><input type="checkbox" bind:checked={done[id]} /> <span>{@html renderInline(boldQty(autoLink(it.text, entry.path)))}</span></label>
            </li>
          {/each}
        </ul>
      {/each}
    </section>
    <section class="steps" class:hide={tab !== 'steps'}>
      <h2>Steps</h2>
      {#each r.steps as g}
        {#if g.name}<h3>{g.name}</h3>{/if}
        <ol>
          {#each g.items as it}
            {@const id = 's' + it.line}
            <li class:done={done[id]} class:current={it.line === nextStep}>
              <label><input type="checkbox" bind:checked={done[id]} /> <span>{@html renderInline(strip(it.text))}</span></label>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="muted">No steps found in this recipe.</p>
      {/each}
    </section>
  </div>
</div>

<style>
  .cook { position: fixed; inset: 0; z-index: 60; background: var(--bg); overflow-y: auto; padding: calc(12px + env(safe-area-inset-top)) 24px 40px; font-family: var(--serif); }
  header { position: sticky; top: 0; background: var(--bg); padding-bottom: 12px; border-bottom: 1px solid var(--border); }
  h1 { margin: 0; font-size: 1.7rem; }
  .pill { background: var(--accent-soft); color: var(--accent-ink); padding: 2px 12px; border-radius: 12px; font-family: var(--sans); }
  .cols { display: grid; grid-template-columns: minmax(260px, 1fr) 2fr; gap: 40px; margin-top: 16px; }
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
    .cook { padding: calc(12px + env(safe-area-inset-top)) 16px 40px; }
    .cols { grid-template-columns: 1fr; gap: 0; }
    .tabs { display: flex; }
    .hide { display: none; }
    h1 { font-size: 1.3rem; }
    li { font-size: 20px; } .steps li { font-size: 21px; }
  }
</style>
