<script lang="ts">
  import Icon from './Icon.svelte';
  import Tree, { buildTree } from './Tree.svelte';
  import { app, go, canWrite } from '../lib/state.svelte';
  import { newNote, newFolder, upload, moveTo } from '../lib/ops';

  const tree = $derived(buildTree(app.folders, app.files));
  const currentDir = $derived(app.route.view === 'folder' ? app.route.path : app.route.path.includes('/') ? app.route.path.slice(0, app.route.path.lastIndexOf('/')) : '');
  let dropRoot = $state(false);
</script>

<div class="side">
  <div class="brand row">
    <a href="#/" class="logo" onclick={() => (app.sidebar = false)}><img src="/icon.svg" alt="" width="24" height="24" /> Recipes</a>
    <span class="spacer"></span>
    <button class="btn ghost icon" title="New recipe (⌘N)" aria-label="New recipe" onclick={() => newNote(currentDir)}><Icon name="plus" /></button>
    <button class="btn ghost icon" title="New folder" aria-label="New folder" onclick={() => newFolder(currentDir)}><Icon name="folder-plus" /></button>
  </div>
  <button class="search btn" onclick={() => (app.switcher = true)}>
    <Icon name="search" size={16} /><span>Search recipes</span><span class="spacer"></span><kbd>⌘K</kbd>
  </button>
  <div
    class="tree"
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
    {#if canWrite()}
      <button class="btn ghost" onclick={() => go('#/import')}><Icon name="download" size={16} /> Import</button>
      <button class="btn ghost" onclick={() => go('#/trash')}><Icon name="trash" size={16} /> Trash</button>
    {:else if app.session.pin_set}
      <button class="btn ghost" onclick={() => (app.pinPrompt = true)}><Icon name="lock" size={16} /> Unlock editing</button>
    {/if}
    <span class="spacer"></span>
    <button class="btn ghost icon" title="Settings" aria-label="Settings" onclick={() => go('#/settings')}><Icon name="settings" /></button>
  </div>
</div>

<style>
  .side { display: flex; flex-direction: column; height: 100%; gap: 8px; padding: 10px 8px calc(8px + env(safe-area-inset-bottom)); }
  .brand { padding: 2px 4px; flex-wrap: nowrap; }
  .logo { display: flex; align-items: center; gap: 8px; font-family: var(--serif); font-weight: 700; font-size: 19px; text-decoration: none; color: var(--text); }
  .search { justify-content: flex-start; color: var(--muted); background: var(--surface); }
  kbd { font: 11px var(--mono); border: 1px solid var(--border); border-radius: 4px; padding: 1px 5px; }
  .tree { flex: 1; overflow-y: auto; overscroll-behavior: contain; border-radius: 8px; }
  .tree.drop { outline: 2px dashed var(--accent); }
  .pad { padding: 8px; }
  .foot { display: flex; gap: 4px; border-top: 1px solid var(--border); padding-top: 8px; flex-wrap: wrap; }
</style>
