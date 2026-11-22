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

  /** Paths of the virtual "Favorites" and "Bookmarks" groups at the top of the tree (not real folders). */
  export const FAV = '\u0000favorites';
  export const MINE = '\u0000mine';
  const isVirtual = (p?: string) => !!p?.startsWith('\u0000');

  const saved = (() => { try { return JSON.parse(localStorage.getItem('expanded') || '[]'); } catch { return []; } })();
  export const expanded = $state<Record<string, boolean>>(Object.fromEntries(saved.map((p: string) => [p, true])));
  expanded[FAV] = (() => { try { return localStorage.getItem('favOpen') !== '0'; } catch { return true; } })();
  expanded[MINE] = (() => { try { return localStorage.getItem('mineOpen') !== '0'; } catch { return true; } })();
  export function persistExpanded() {
    try {
      localStorage.setItem('expanded', JSON.stringify(Object.keys(expanded).filter((k) => expanded[k] && !isVirtual(k))));
      localStorage.setItem('favOpen', expanded[FAV] ? '1' : '0');
      localStorage.setItem('mineOpen', expanded[MINE] ? '1' : '0');
    } catch {}
  }

  /** A folder just opened or closed by a click in the column: navigating to it keeps that state. */
  let clicked: string | null = null;

  /** Expand the folders on the way to `path` (and `path` itself when it is a folder); fold the rest. */
  export function reveal(path: string, isFolder: boolean) {
    const parts = path.split('/');
    const asClicked = isFolder && clicked === path;
    clicked = null;
    const upto = isFolder && !(asClicked && !expanded[path]) ? parts.length : parts.length - 1;
    const keep = new Set(Array.from({ length: upto }, (_, i) => parts.slice(0, i + 1).join('/')));
    for (const k of Object.keys(expanded)) if (!isVirtual(k) && expanded[k] && !keep.has(k)) expanded[k] = false;
    for (const k of keep) expanded[k] = true;
    persistExpanded();
  }
</script>

<script lang="ts">
  import Tree from './Tree.svelte';
  import { app, go, noteHash, folderHash, canWrite, dirOf, nameOf, newTab, wide } from '../lib/state.svelte';
  import { openMenu } from '../lib/menu.svelte';
  import { moveTo, upload, entryItems } from '../lib/ops';
  import { iconOf } from '../lib/icons';
  import CardIcon from './CardIcon.svelte';
  import Icon from './Icon.svelte';

  let { node, depth = 0 }: { node: TreeNode; depth?: number } = $props();
  let dropOver = $state('');

  const DRAG = 'application/x-recipes-path';

  function toggle(n: TreeNode) {
    expanded[n.path] = !expanded[n.path];
    persistExpanded();
  }

  /** A folder row opens and closes its list of files and shows the folder's page; a file row opens the file. */
  function open(n: TreeNode, e?: MouseEvent) {
    if (isVirtual(n.path)) return toggle(n);
    if (e && (e.metaKey || e.ctrlKey) && wide()) return newTab(n.folder ? folderHash(n.path) : noteHash(n.path));
    if (n.folder) {
      toggle(n);
      clicked = n.path;
      go(folderHash(n.path));
    } else go(noteHash(n.path));
  }

  const items = (n: TreeNode) => entryItems(n.path, n.folder);

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

  const icon = (n: TreeNode) => (n.kind === 'pdf' ? 'file' : n.kind === 'image' ? 'image' : n.kind === 'other' ? 'paperclip' : '');
  const folderIcon = (n: TreeNode) => (n.path === FAV ? { icon: 'bookmark' } : n.path === MINE ? { icon: 'star' } : iconOf(n.name, app.folderMeta[n.path]?.emoji));
  // same rule as the recipe cards: the note's own icon, else its folder's
  const noteIcon = (n: TreeNode) => {
    const e = app.byPath[n.path];
    const d = dirOf(n.path);
    return e ? iconOf(e.title, e.frontmatter?.emoji, d ? iconOf(nameOf(d), app.folderMeta[d]?.emoji) : null) : null;
  };
  const label = (n: TreeNode) => (n.folder ? n.name : n.kind === 'note' ? n.name.replace(/\.md$/i, '') : n.name);
</script>

{#each node.children as n, i (n.path)}
  {@const active = app.route.path === n.path && (n.folder ? app.route.view === 'folder' : app.route.view === 'note')}
  {@const virtual = isVirtual(n.path)}
  {@const ic = n.folder ? folderIcon(n) : n.kind === 'note' ? noteIcon(n) : null}
  <div
    class="row-item"
    class:active
    class:folder={n.folder}
    class:drop={dropOver === n.path}
    class:dim={n.path === 'attachments'}
    style="padding-left:{8 + depth * 16}px"
    draggable={canWrite() && !virtual}
    role="treeitem"
    aria-selected={active}
    aria-current={active ? 'page' : undefined}
    aria-expanded={n.folder ? !!expanded[n.path] : undefined}
    tabindex="0"
    ondragstart={(e) => e.dataTransfer?.setData(DRAG, n.path)}
    ondragover={(e) => { if (n.folder && !virtual) { e.preventDefault(); dropOver = n.path; } }}
    ondragleave={() => (dropOver = '')}
    ondrop={(e) => n.folder && !virtual && onDrop(e, n)}
    onclick={(e) => open(n, e)}
    onkeydown={(e) => { if (e.key === 'Enter') open(n); if (n.folder && (e.key === 'ArrowRight' || e.key === 'ArrowLeft')) { expanded[n.path] = e.key === 'ArrowRight'; persistExpanded(); } }}
    oncontextmenu={(e) => { if (!virtual) openMenu(e, items(n)); }}
  >
    {#if n.folder}
      <button class="chev" class:open={expanded[n.path]} aria-label="Toggle folder" tabindex="-1" onclick={(e) => { e.stopPropagation(); toggle(n); }}><Icon name="chevron-right" size={16} /></button>
    {:else}
      <!-- a file keeps the chevron's space, so it sits indented under its folder -->
      <span class="chev-space">{#if !ic && icon(n)}<Icon name={icon(n)} size={16} />{/if}</span>
    {/if}
    {#if ic}<span class="ic"><CardIcon icon={ic} size={16} /></span>{/if}
    <span class="name">{label(n)}</span>
    {#if !virtual}<button class="more" aria-label="More actions" tabindex="-1" onclick={(e) => openMenu(e, items(n))}>⋯</button>{/if}
    {#if n.folder}<span class="count">{n.children.filter((c) => !c.folder).length || ''}</span>{/if}
  </div>
  {#if n.folder && expanded[n.path]}
    <Tree node={n} depth={depth + 1} />
  {/if}
  {#if virtual && !isVirtual(node.children[i + 1]?.path)}<div class="fav-gap" aria-hidden="true"></div>{/if}
{/each}

<style>
  .row-item {
    display: flex; align-items: center; gap: 4px; padding: 2px 8px; border-radius: 6px; cursor: pointer;
    min-height: 32px; user-select: none; color: var(--text);
  }
  .row-item:hover { background: var(--sunken); }
  .fav-gap { height: 12px; } /* Favorites and Bookmarks stand apart from the folders */
  .row-item.active { background: var(--accent-soft); }
  .row-item.drop { outline: 2px dashed var(--accent); }
  .row-item.dim { opacity: 0.6; }
  .folder .name { font-weight: 500; }
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chev { display: inline-flex; border: 0; background: none; width: 16px; padding: 0; cursor: pointer; color: var(--muted); transition: transform 0.1s; }
  .chev.open { transform: rotate(90deg); }
  .chev-space { display: inline-flex; width: 16px; flex: none; color: var(--muted); }
  .ic { display: inline-flex; width: 16px; justify-content: center; flex: none; }
  .folder .ic { width: auto; }
  .count { color: var(--muted); font-size: 12px; }
  .more { border: 0; background: none; color: var(--muted); padding: 0 4px; cursor: pointer; opacity: 0; }
  .row-item:hover .more, .row-item.active .more { opacity: 1; }
  @media (hover: none) { .more { opacity: 1; } .row-item { min-height: 40px; } }
</style>
