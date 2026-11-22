<script lang="ts">
  import Icon from './Icon.svelte';
  import Tree, { buildTree, FAV, MINE, reveal, type TreeNode } from './Tree.svelte';
  import EditingAs from './EditingAs.svelte';
  import { app, go, favorites } from '../lib/state.svelte';
  import type { Entry } from '../lib/types';
  import { upload, moveTo } from '../lib/ops';
  import { openSearch } from './Switcher.svelte';
  import { tick, untrack } from 'svelte';

  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
  const tree = $derived.by(() => {
    const t = buildTree(app.folders, app.files);
    const rows = (fs: Entry[]): TreeNode[] => fs.sort((a, b) => collator.compare(a.title, b.title))
      .map((f) => ({ name: f.path.slice(f.path.lastIndexOf('/') + 1), path: f.path, folder: false, children: [], kind: f.kind }));
    const favs = rows(app.files.filter((f) => f.kind === 'note' && f.frontmatter?.favorite === true));
    const mine = rows(favorites.paths.map((p) => app.byPath[p]).filter((f): f is Entry => f?.kind === 'note'));
    if (favs.length) t.children.unshift({ name: 'Bookmarks', path: FAV, folder: true, children: favs });
    if (mine.length) t.children.unshift({ name: 'Favorites', path: MINE, folder: true, children: mine });
    return t;
  });
  let dropRoot = $state(false);
  let treeEl: HTMLDivElement;

  // follow the page being opened: open its folders and scroll its row into view
  // (not on switching tabs, and not when a folder is toggled)
  $effect(() => {
    app.nav;
    if (!app.loaded) return;
    const { view, path } = untrack(() => app.route);
    if (view !== 'note' && view !== 'folder') return;
    untrack(() => reveal(path, view === 'folder'));
    tick().then(() => {
      const rows = treeEl?.querySelectorAll<HTMLElement>('[aria-current="page"]');
      // the last match is the row in the folder tree (an earlier one may be in Favorites or Bookmarks)
      rows?.[rows.length - 1]?.scrollIntoView({ block: 'nearest' });
    });
  });
</script>

<div class="side">
  <div class="brand row">
    <a href="#/" class="logo" onclick={() => (app.sidebar = false)}><img src="/icon.svg" alt="" width="24" height="24" /> Recipes</a>
    <span class="spacer"></span>
    <button class="btn ghost icon" aria-label="Search" title="Search (⌘K)" onclick={openSearch}><Icon name="search" /></button>
  </div>
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
    <button class="btn ghost icon" class:on={app.route.view === 'ingredients'} title="Ingredients" aria-label="Ingredients" onclick={() => go('#/ingredients')}><Icon name="carrot" /></button>
    <button class="btn ghost icon" class:on={app.route.view === 'calendar'} title="Calendar" aria-label="Calendar" onclick={() => go('#/calendar')}><Icon name="calendar" /></button>
    {#if app.session.role === 'admin'}<EditingAs />{/if}
    <span class="spacer"></span>
    <button class="btn ghost icon" title="Settings" aria-label="Settings" onclick={() => go('#/settings')}><Icon name="settings" /></button>
  </div>
</div>

<style>
  .side { display: flex; flex-direction: column; height: 100%; gap: 8px; padding: 12px 8px calc(8px + env(safe-area-inset-bottom)); }
  .brand { padding: 2px 4px; flex-wrap: nowrap; min-height: 36px; }
  .logo { display: flex; align-items: center; gap: 8px; font-family: var(--serif); font-weight: 600; font-size: 19px; text-decoration: none; color: var(--text); }
  .tree { flex: 1; overflow-y: auto; border-radius: 8px; }
  .tree.drop { outline: 2px dashed var(--accent); }
  .pad { padding: 8px; }
  .foot { display: flex; gap: 4px; border-top: 1px solid var(--border); padding-top: 8px; flex-wrap: wrap; }
</style>
