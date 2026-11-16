<script lang="ts">
  import Icon from './Icon.svelte';
  import CardIcon from './CardIcon.svelte';
  import { app, selectTab, newTab, closeTab, closeOtherTabs, moveTab, nameOf, dirOf, type Tab } from '../lib/state.svelte';
  import { iconOf } from '../lib/icons';
  import { openMenu } from '../lib/menu.svelte';

  const NAMES: Record<string, [string, string]> = {
    settings: ['Settings', 'settings'], import: ['Import', 'plus'], ingredients: ['Ingredients', 'carrot'], search: ['Search', 'search'],
  };

  /** what a tab shows: the page's name and icon, as the folder column shows them */
  function label(t: Tab): { title: string; card?: ReturnType<typeof iconOf>; icon?: string } {
    const { view, path } = t.route;
    const folderIcon = (d: string) => iconOf(nameOf(d), app.folderMeta[d]?.emoji);
    if (view === 'note') {
      const e = app.byPath[path];
      const d = dirOf(path);
      const title = e?.title ?? nameOf(path).replace(/\.md$/i, '');
      return { title, card: e?.kind === 'note' ? iconOf(title, e.frontmatter?.emoji, d ? folderIcon(d) : null) : null, icon: e?.kind === 'note' ? undefined : 'file' };
    }
    if (view === 'folder') return path ? { title: nameOf(path), card: folderIcon(path) } : { title: 'Recipes', icon: 'book' };
    const [title, icon] = NAMES[view] ?? ['Recipes', 'book'];
    return { title, icon };
  }

  const DRAG = 'application/x-recipes-tab';
  let over = $state<number | null>(null);

  function menu(e: MouseEvent, t: Tab) {
    openMenu(e, [
      { label: 'Close', run: () => closeTab(t.id) },
      ...(app.tabs.length > 1 ? [{ label: 'Close other tabs', run: () => closeOtherTabs(t.id) }] : []),
      { label: 'New tab', run: () => newTab(), sep: true },
    ]);
  }
</script>

<div class="tabs no-print" role="tablist" aria-label="Open pages">
  {#each app.tabs as t, i (t.id)}
    {@const l = label(t)}
    <div
      class="tab"
      class:on={t.id === app.tab}
      class:over={over === i}
      role="tab"
      tabindex="0"
      aria-selected={t.id === app.tab}
      title={l.title}
      draggable="true"
      onclick={() => selectTab(t.id)}
      onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectTab(t.id); } }}
      onmousedown={(e) => { if (e.button === 1) e.preventDefault(); }}
      onauxclick={(e) => { if (e.button === 1) { e.preventDefault(); closeTab(t.id); } }}
      oncontextmenu={(e) => menu(e, t)}
      ondragstart={(e) => e.dataTransfer?.setData(DRAG, String(t.id))}
      ondragover={(e) => { if (e.dataTransfer?.types.includes(DRAG)) { e.preventDefault(); over = i; } }}
      ondragleave={() => (over = null)}
      ondrop={(e) => { e.preventDefault(); over = null; const id = Number(e.dataTransfer?.getData(DRAG)); if (id) moveTab(id, i); }}
    >
      <span class="ic">{#if l.card}<CardIcon icon={l.card} size={14} />{:else if l.icon}<Icon name={l.icon} size={14} />{/if}</span>
      <span class="t">{l.title}</span>
      <button class="x" aria-label="Close tab" title="Close tab" tabindex="-1" onclick={(e) => { e.stopPropagation(); closeTab(t.id); }}><Icon name="x" size={14} /></button>
    </div>
  {/each}
  <button class="add" aria-label="New tab" title="New tab (⌘T)" onclick={() => newTab()}><Icon name="plus" size={16} /></button>
</div>

<style>
  /* the window's colour, like the title bar above; the active tab is the page's colour and runs into it */
  .tabs {
    flex: none; display: flex; align-items: flex-end; gap: 2px; padding: 6px 8px 0; background: var(--sunken);
    overflow-x: auto; overflow-y: hidden;
  }
  .tab {
    flex: 0 1 200px; min-width: 96px; height: 34px; display: flex; align-items: center; gap: 6px; padding: 0 6px 0 12px;
    border-radius: 8px 8px 0 0; color: var(--muted); font-size: 13px; cursor: default; user-select: none;
  }
  .tab:hover { color: var(--text); background: color-mix(in srgb, var(--bg) 55%, transparent); }
  .tab.on { background: var(--bg); color: var(--text); }
  .tab.over { box-shadow: inset 2px 0 0 var(--accent); }
  .tab:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .ic { display: inline-flex; flex: none; }
  .ic:empty { display: none; }
  .t { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x, .add { flex: none; display: grid; place-items: center; border: 0; background: none; color: var(--muted); border-radius: 6px; cursor: pointer; padding: 0; }
  .x { width: 22px; height: 22px; opacity: 0; }
  .tab:hover .x, .tab.on .x { opacity: 1; }
  .x:hover, .add:hover { background: var(--sunken); color: var(--text); }
  .tab:not(.on) .x:hover { background: var(--bg); }
  .add { width: 30px; height: 30px; margin: 0 0 2px 4px; }
  /* phones show one page at a time */
  @media (max-width: 899px) { .tabs { display: none; } }
</style>
