<script lang="ts">
  import Icon from './Icon.svelte';
  import Tree, { buildTree, FAV, reveal } from './Tree.svelte';
  import { app, go, canWrite } from '../lib/state.svelte';
  import { upload, moveTo } from '../lib/ops';
  import { openSearch } from './Switcher.svelte';
  import { tick, untrack } from 'svelte';

  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
  const tree = $derived.by(() => {
    const t = buildTree(app.folders, app.files);
    const favs = app.files.filter((f) => f.kind === 'note' && f.frontmatter?.favorite === true)
      .sort((a, b) => collator.compare(a.title, b.title))
      .map((f) => ({ name: f.path.slice(f.path.lastIndexOf('/') + 1), path: f.path, folder: false, children: [], kind: f.kind }));
    if (favs.length) t.children.unshift({ name: 'Favorites', path: FAV, folder: true, children: favs });
    return t;
  });
  let dropRoot = $state(false);
  let treeEl: HTMLDivElement;

  // follow the page being shown: open its folders and scroll its row into view
  $effect(() => {
    const { view, path } = app.route;
    if (!app.loaded || (view !== 'note' && view !== 'folder')) return;
    untrack(() => reveal(path, view === 'folder')); // only on navigation, not when a folder is toggled
    tick().then(() => {
      const rows = treeEl?.querySelectorAll<HTMLElement>('[aria-current="page"]');
      // the last match is the row in the folder tree (an earlier one may be in Favorites)
      rows?.[rows.length - 1]?.scrollIntoView({ block: 'nearest' });
    });
  });
</script>

<div class="side">
  <div class="brand row">
    <a href="#/" class="logo" onclick={() => (app.sidebar = false)}><img src="/icon.svg" alt="" width="24" height="24" /> Recipes</a>
  </div>
  <button class="search btn" onclick={openSearch}>
    <Icon name="search" size={16} /><span>Search recipes</span><span class="spacer"></span><kbd>⌘K</kbd>
  </button>
  <div
    class="tree"
    bind:this={treeEl}
    class:drop={dropRoot}
    role="tree"
    tabindex="-1"
    ondragover={(e) => { e.preventDefault(); dropRoot = true; }}
    ondragleave={() => (dropRoot = false)}
    ondrop={(e) => {
      e.preventDefault(); dropRoot = false;
      const from = e.dataTransfer?.getData('application/x-recipes-path');
      if (from && from.includes('/')) moveTo(from, from.slice(from.lastIndexOf('/') + 1), app.folders.includes(from));
      else if (!from && e.dataTransfer?.files.length) upload('', e.dataTransfer.files);
    }}
  >
    {#if !app.loaded}<p class="muted small pad">Loading…</p>{/if}
    <Tree node={tree} />
  </div>
  <div class="foot">
    {#if !canWrite() && app.session.pin_set}
      <button class="btn ghost" onclick={() => (app.pinPrompt = true)}><Icon name="lock" size={16} /> Unlock editing</button>
    {/if}
    <button class="btn ghost" class:on={app.route.view === 'ingredients'} onclick={() => go('#/ingredients')}><Icon name="carrot" size={16} /> Ingredients</button>
    <span class="spacer"></span>
    <button class="btn ghost icon" title="Settings" aria-label="Settings" onclick={() => go('#/settings')}><Icon name="settings" /></button>
  </div>
</div>

<style>
  .side { display: flex; flex-direction: column; height: 100%; gap: 8px; padding: 12px 8px calc(8px + env(safe-area-inset-bottom)); }
  .brand { padding: 2px 4px; flex-wrap: nowrap; min-height: 36px; }
  .logo { display: flex; align-items: center; gap: 8px; font-family: var(--serif); font-weight: 600; font-size: 19px; text-decoration: none; color: var(--text); }
  .search { justify-content: flex-start; color: var(--muted); background: var(--surface); }
  kbd { font: 11px var(--mono); border: 1px solid var(--border); border-radius: 4px; padding: 2px 4px; }
  .tree { flex: 1; overflow-y: auto; overscroll-behavior: contain; border-radius: 8px; }
  .tree.drop { outline: 2px dashed var(--accent); }
  .pad { padding: 8px; }
  .foot { display: flex; gap: 4px; border-top: 1px solid var(--border); padding-top: 8px; flex-wrap: wrap; }
</style>
