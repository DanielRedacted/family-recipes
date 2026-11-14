<script lang="ts" module>
  export interface TreeNode { name: string; path: string; folder: boolean; children: TreeNode[]; kind?: string }

  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

  export function buildTree(folders: string[], files: { path: string; kind: string }[]): TreeNode {
    const root: TreeNode = { name: '', path: '', folder: true, children: [] };
    const dirs = new Map<string, TreeNode>([['', root]]);
    const dir = (p: string): TreeNode => {
      if (dirs.has(p)) return dirs.get(p)!;
      const parent = dir(p.includes('/') ? p.slice(0, p.lastIndexOf('/')) : '');
      const n: TreeNode = { name: p.slice(p.lastIndexOf('/') + 1), path: p, folder: true, children: [] };
      parent.children.push(n);
      dirs.set(p, n);
      return n;
    };
    folders.forEach(dir);
    for (const f of files) {
      const d = f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '';
      dir(d).children.push({ name: f.path.slice(f.path.lastIndexOf('/') + 1), path: f.path, folder: false, children: [], kind: f.kind });
    }
    const sort = (n: TreeNode) => {
      n.children.sort((a, b) => (a.folder !== b.folder ? (a.folder ? -1 : 1)
        : a.path === 'attachments' ? 1 : b.path === 'attachments' ? -1 : collator.compare(a.name, b.name)));
      n.children.forEach(sort);
    };
    sort(root);
    return root;
  }

  const saved = (() => { try { return JSON.parse(localStorage.getItem('expanded') || '[]'); } catch { return []; } })();
  export const expanded = $state<Record<string, boolean>>(Object.fromEntries(saved.map((p: string) => [p, true])));
  export function persistExpanded() {
    try { localStorage.setItem('expanded', JSON.stringify(Object.keys(expanded).filter((k) => expanded[k]))); } catch {}
  }
</script>

<script lang="ts">
  import Tree from './Tree.svelte';
  import { app, go, noteHash, folderHash, canWrite } from '../lib/state.svelte';
  import { openMenu } from '../lib/menu.svelte';
  import { newNote, newFolder, rename, moveDialog, remove, moveTo, upload, shell } from '../lib/ops';
  import { downloadUrl } from '../lib/api';

  let { node, depth = 0 }: { node: TreeNode; depth?: number } = $props();
  let dropOver = $state('');

  const DRAG = 'application/x-recipes-path';

  function toggle(n: TreeNode) {
    expanded[n.path] = !expanded[n.path];
    persistExpanded();
  }

  function open(n: TreeNode) {
    if (n.folder) {
      expanded[n.path] = true;
      persistExpanded();
      go(folderHash(n.path));
    } else go(noteHash(n.path));
  }

  function items(n: TreeNode) {
    const it = [];
    if (n.folder) {
      it.push({ label: 'New recipe here', run: () => newNote(n.path) }, { label: 'New folder here', run: () => newFolder(n.path) });
    }
    it.push(
      { label: 'Rename…', run: () => rename(n.path, n.folder) },
      { label: 'Move to…', run: () => moveDialog(n.path, n.folder) },
      { label: n.folder ? 'Download as zip' : 'Download', run: () => (location.href = downloadUrl(n.path)) },
    );
    if (app.session.shell) it.push({ label: 'Show in Finder', run: () => shell('reveal', { path: n.path }) });
    it.push({ label: 'Move to trash', run: () => remove(n.path, n.folder), danger: true });
    return it;
  }

  function onDrop(e: DragEvent, target: TreeNode) {
    e.preventDefault();
    e.stopPropagation();
    dropOver = '';
    const from = e.dataTransfer?.getData(DRAG);
    const dir = target.folder ? target.path : target.path.slice(0, Math.max(0, target.path.lastIndexOf('/')));
    if (from) {
      const name = from.slice(from.lastIndexOf('/') + 1);
      if (dir === from || dir.startsWith(from + '/')) return;
      moveTo(from, dir ? `${dir}/${name}` : name, app.folders.includes(from));
    } else if (e.dataTransfer?.files.length) {
      upload(dir, e.dataTransfer.files);
    }
  }

  const icon = (n: TreeNode) => (n.kind === 'pdf' ? '📄' : n.kind === 'image' ? '🖼' : n.kind === 'other' ? '📎' : '');
  const label = (n: TreeNode) => (n.folder ? n.name : n.kind === 'note' ? n.name.replace(/\.md$/i, '') : n.name);
</script>

{#each node.children as n (n.path)}
  {@const active = app.route.path === n.path}
  <div
    class="row-item"
    class:active
    class:folder={n.folder}
    class:drop={dropOver === n.path}
    class:dim={n.path === 'attachments'}
    style="padding-left:{8 + depth * 14}px"
    draggable={canWrite()}
    role="treeitem"
    aria-selected={active}
    aria-expanded={n.folder ? !!expanded[n.path] : undefined}
    tabindex="0"
    ondragstart={(e) => e.dataTransfer?.setData(DRAG, n.path)}
    ondragover={(e) => { if (n.folder) { e.preventDefault(); dropOver = n.path; } }}
    ondragleave={() => (dropOver = '')}
    ondrop={(e) => n.folder && onDrop(e, n)}
    onclick={() => open(n)}
    onkeydown={(e) => { if (e.key === 'Enter') open(n); if (n.folder && (e.key === 'ArrowRight' || e.key === 'ArrowLeft')) { expanded[n.path] = e.key === 'ArrowRight'; persistExpanded(); } }}
    oncontextmenu={(e) => openMenu(e, items(n))}
  >
    {#if n.folder}
      <button class="chev" class:open={expanded[n.path]} aria-label="Toggle folder" tabindex="-1" onclick={(e) => { e.stopPropagation(); toggle(n); }}>›</button>
    {:else}
      <span class="chev-space">{icon(n)}</span>
    {/if}
    <span class="name">{label(n)}</span>
    {#if n.folder}<span class="count">{n.children.filter((c) => !c.folder).length || ''}</span>{/if}
    <button class="more" aria-label="More actions" tabindex="-1" onclick={(e) => openMenu(e, items(n))}>⋯</button>
  </div>
  {#if n.folder && expanded[n.path]}
    <Tree node={n} depth={depth + 1} />
  {/if}
{/each}

<style>
  .row-item {
    display: flex; align-items: center; gap: 4px; padding: 3px 6px; border-radius: 6px; cursor: pointer;
    min-height: 30px; user-select: none; color: var(--text);
  }
  .row-item:hover { background: var(--sunken); }
  .row-item.active { background: var(--accent-soft); }
  .row-item.drop { outline: 2px dashed var(--accent); }
  .row-item.dim { opacity: 0.6; }
  .folder .name { font-weight: 500; }
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chev { border: 0; background: none; width: 18px; padding: 0; cursor: pointer; color: var(--muted); font-size: 16px; transition: transform 0.1s; }
  .chev.open { transform: rotate(90deg); }
  .chev-space { width: 18px; font-size: 11px; text-align: center; }
  .count { color: var(--muted); font-size: 12px; }
  .more { border: 0; background: none; color: var(--muted); padding: 0 4px; cursor: pointer; opacity: 0; }
  .row-item:hover .more, .row-item.active .more { opacity: 1; }
  @media (hover: none) { .more { opacity: 1; } .row-item { min-height: 40px; } }
</style>
