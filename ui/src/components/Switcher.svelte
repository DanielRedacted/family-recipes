<script lang="ts">
  import { app, go, noteHash, folderHash } from '../lib/state.svelte';
  import { search } from '../lib/search';
  import { catalog, loadIngredients } from '../lib/ingredients.svelte';

  const dir = (p: string) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/')) : '');
  let q = $state('');
  let sel = $state(0);
  let input: HTMLInputElement | undefined = $state();

  const folderHits = $derived(q.trim() ? app.folders.filter((f) => f.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 5) : []);
  const hits = $derived(q.trim() ? search(q, 40) : [...app.files].filter((f) => f.kind === 'note').sort((a, b) => b.mtime - a.mtime).slice(0, 10).map((f) => ({ path: f.path, title: f.title })));
  const ingHits = $derived(q.trim().length >= 2 ? catalog.items.filter((i) => i.name.toLowerCase().includes(q.trim().toLowerCase())).sort((a, b) => b.count - a.count).slice(0, 3) : []);
  const all = $derived([
    ...folderHits.map((f) => ({ path: f, title: f, folder: true, ing: false, sub: '' })),
    ...hits.map((h) => ({ ...h, folder: false, ing: false, sub: dir(h.path) })),
    ...ingHits.map((i) => ({ path: i.name, title: i.name, folder: false, ing: true, sub: `ingredient · ${i.count} recipe${i.count === 1 ? '' : 's'}` })),
  ]);

  $effect(() => {
    if (app.switcher) {
      loadIngredients();
      q = '';
      sel = 0;
      setTimeout(() => input?.focus(), 0);
    }
  });
  $effect(() => { q; sel = 0; });

  function pick(i: number) {
    const h = all[i];
    if (!h) return;
    app.switcher = false;
    go(h.ing ? '#/ingredients/' + encodeURIComponent(h.path) : h.folder ? folderHash(h.path) : noteHash(h.path));
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { sel = Math.min(sel + 1, all.length - 1); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { sel = Math.max(sel - 1, 0); e.preventDefault(); }
    else if (e.key === 'Enter') { pick(sel); e.preventDefault(); }
    else if (e.key === 'Escape') app.switcher = false;
  }
</script>

{#if app.switcher}
  <div class="back" role="presentation" onclick={() => (app.switcher = false)}>
    <div class="box card" role="dialog" aria-modal="true" aria-label="Search" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={key}>
      <input class="field q" placeholder="Search titles, ingredients, notes…" bind:value={q} bind:this={input} aria-label="Search" />
      <ul role="listbox">
        {#each all as h, i (h.path + h.folder + h.ing)}
          <li role="option" aria-selected={i === sel}>
            <button class:sel={i === sel} onmouseenter={() => (sel = i)} onclick={() => pick(i)}>
              <span>{h.folder ? '🗀 ' : h.ing ? '🥕 ' : ''}{h.title}</span>
              {#if !h.folder}<span class="muted small">{h.sub}</span>{/if}
            </button>
          </li>
        {:else}
          <li class="muted pad">{q ? 'Nothing found' : 'Start typing to search'}</li>
        {/each}
      </ul>
      <p class="muted small hint">↑↓ to move · Enter to open · Esc to close</p>
    </div>
  </div>
{/if}

<style>
  .back { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.3); z-index: 80; display: flex; justify-content: center; align-items: flex-start; padding: 10vh 12px 12px; }
  .box { width: min(640px, 100%); max-height: 75vh; display: flex; flex-direction: column; overflow: hidden; }
  .q { border: 0 !important; border-bottom: 1px solid var(--border) !important; border-radius: 0; font-size: 17px; padding: 14px 16px; outline: none !important; }
  ul { list-style: none; margin: 0; padding: 6px; overflow-y: auto; }
  li button { width: 100%; display: flex; justify-content: space-between; gap: 12px; text-align: left; border: 0; background: none; padding: 9px 10px; border-radius: 6px; cursor: pointer; }
  li button.sel { background: var(--accent-soft); }
  .pad { padding: 12px; }
  .hint { margin: 0; padding: 6px 12px 10px; border-top: 1px solid var(--border); }
  @media (max-width: 899px) { .back { padding-top: 12px; } .box { max-height: calc(100vh - 24px); } .hint { display: none; } }
</style>
