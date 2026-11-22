<script lang="ts" module>
  import { flushSync } from 'svelte';
  import { app as appState } from '../lib/state.svelte';

  /** Open search with its field focused in the same tap: iOS shows the keyboard only for a focus made
   * while handling the tap itself, so the field is rendered at once (flushSync) and focused here. */
  export function openSearch() {
    appState.switcher = true;
    flushSync();
    document.querySelector<HTMLInputElement>('input[aria-label="Search"]')?.focus();
  }
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import { app, go, noteHash, folderHash } from '../lib/state.svelte';
  import { search } from '../lib/search';
  import { catalog, loadIngredients } from '../lib/ingredients.svelte';
  import { ingredientIcon } from '../lib/icons';
  import { modal } from '../lib/modal';
  import { openFeedback, feedbackWords } from './Feedback.svelte';

  const dir = (p: string) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/')) : '');
  let q = $state('');
  let sel = $state(0);
  let input: HTMLInputElement | undefined = $state();

  const folderHits = $derived(q.trim() ? app.folders.filter((f) => f.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 5) : []);
  const hits = $derived(q.trim() ? search(q, 40) : [...app.files].filter((f) => f.kind === 'note')
    .sort((a, b) => Number(b.frontmatter?.favorite === true) - Number(a.frontmatter?.favorite === true) || b.mtime - a.mtime).slice(0, 10).map((f) => ({ path: f.path, title: f.title })));
  const ingHits = $derived(q.trim().length >= 2 ? catalog.items.filter((i) => i.name.toLowerCase().includes(q.trim().toLowerCase())).sort((a, b) => b.count - a.count).slice(0, 3) : []);
  const all = $derived([
    ...(feedbackWords.test(q.trim()) ? [{ path: '', title: 'Send feedback…', folder: false, ing: false, fb: true, sub: 'a bug, an idea or a note' }] : []),
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
    if ('fb' in h) return openFeedback();
    go(h.ing ? '#/ingredients/' + encodeURIComponent(h.path) : h.folder ? folderHash(h.path) : noteHash(h.path));
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { sel = Math.min(sel + 1, all.length - 1); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { sel = Math.max(sel - 1, 0); e.preventDefault(); }
    else if (e.key === 'Enter') { pick(sel); e.preventDefault(); }
  }
</script>

{#if app.switcher}
  <dialog class="modal-host switcher" aria-label="Search" use:modal={{ onclose: () => (app.switcher = false), outside: true }} onkeydown={key}>
    <div class="box card">
      <input class="field q" placeholder="Search titles, ingredients, notes…" bind:value={q} bind:this={input} aria-label="Search" />
      <ul role="listbox">
        {#each all as h, i (h.path + h.folder + h.ing)}
          <li role="option" aria-selected={i === sel}>
            <button class:sel={i === sel} onmouseenter={() => (sel = i)} onclick={() => pick(i)}>
              <span class="hit">{#if 'fb' in h}<Icon name="message" size={16} />{:else if h.folder}<Icon name="folder" size={16} />{:else if h.ing}{@const e = (ingredientIcon(h.title) as { emoji?: string } | null)?.emoji}{#if e}{e}{:else}<Icon name="carrot" size={16} />{/if}{/if}{h.title}</span>
              {#if !h.folder}<span class="muted small">{h.sub}</span>{/if}
            </button>
          </li>
        {:else}
          <li class="muted pad">{q ? 'Nothing found' : 'Start typing to search'}</li>
        {/each}
      </ul>
      <p class="muted small hint">↑↓ to move · Enter to open · Esc to close</p>
    </div>
  </dialog>
{/if}

<style>
  .switcher { margin: 10vh auto auto; }
  .switcher::backdrop { background: rgba(0, 0, 0, 0.3); }
  .box { width: min(640px, calc(100vw - 24px)); max-height: 75vh; display: flex; flex-direction: column; overflow: hidden; }
  .q { border: 0 !important; border-bottom: 1px solid var(--border) !important; border-radius: 0; font-size: 17px; padding: 16px; outline: none !important; }
  ul { list-style: none; margin: 0; padding: 8px; overflow-y: auto; }
  li button { width: 100%; display: flex; justify-content: space-between; gap: 12px; text-align: left; border: 0; background: none; padding: 8px 12px; border-radius: 6px; cursor: pointer; }
  li button.sel { background: var(--accent-soft); }
  .hit { display: inline-flex; align-items: center; gap: 8px; }
  .hit :global(svg) { color: var(--muted); flex: none; }
  .pad { padding: 12px; }
  .hint { margin: 0; padding: 8px 12px; border-top: 1px solid var(--border); }
  @media (max-width: 899px) { .switcher { margin-top: max(12px, env(safe-area-inset-top)); } .box { max-height: calc(100vh - 24px); } .hint { display: none; } }
</style>
