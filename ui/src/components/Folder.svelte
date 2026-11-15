<script lang="ts">
  import Icon from './Icon.svelte';
  import { app, noteHash, folderHash, canWrite, nameOf, dirOf } from '../lib/state.svelte';
  import Crumbs from './Crumbs.svelte';
  import CardIcon from './CardIcon.svelte';
  import { fileUrl } from '../lib/api';
  import { vaultTarget } from '../lib/markdown';
  import { plusMenu, entryItems } from '../lib/ops';
  import { openMenu } from '../lib/menu.svelte';
  import { tagsOf } from '../lib/search';
  import { iconOf, cardColor } from '../lib/icons';
  import type { Entry } from '../lib/types';

  let { path }: { path: string } = $props();
  let view = $state<'cards' | 'list'>((() => { try { return (localStorage.getItem('folderView') as any) || 'cards'; } catch { return 'cards'; } })());
  $effect(() => { try { localStorage.setItem('folderView', view); } catch {} });

  const exists = $derived(path === '' || app.folders.includes(path));
  const subfolders = $derived(app.folders.filter((f) => (path ? f.startsWith(path + '/') && !f.slice(path.length + 1).includes('/') : !f.includes('/'))));
  const files = $derived(app.files.filter((f) => (f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '') === path)
    .sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: 'base' })));
  const folderIcon = (dir: string) => iconOf(nameOf(dir), app.folderMeta[dir]?.emoji);
  const noteIcon = (f: Entry) => (f.kind === 'note' ? iconOf(f.title, f.frontmatter?.emoji, dirOf(f.path) ? folderIcon(dirOf(f.path)) : null) : null);
  const favorites = $derived(path ? [] : app.files.filter((f) => f.kind === 'note' && f.frontmatter?.favorite === true)
    .sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: 'base' })));
  const menuFor = (p: string, folder: boolean) => (e: MouseEvent) => openMenu(e, entryItems(p, folder));
  const countIn = (dir: string) => app.files.filter((f) => f.path.startsWith(dir + '/')).length;

  function cover(f: Entry): string | null {
    if (f.kind === 'image') return fileUrl(f.path);
    if (!f.cover) return null;
    if (/^https?:/i.test(f.cover)) return f.cover;
    const p = vaultTarget(f.cover, f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '');
    return p ? fileUrl(p) : null;
  }
  const meta = (f: Entry) => [f.frontmatter?.yield ?? f.frontmatter?.servings, f.frontmatter?.total_time ?? f.frontmatter?.cook_time]
    .filter(Boolean).map(String).join(' · ');
  const badge = (f: Entry) => (f.kind === 'pdf' ? 'PDF' : f.kind === 'image' ? 'Image' : f.kind === 'other' ? nameOf(f.path).split('.').pop()?.toUpperCase() : f.recipe ? '' : 'Note');
</script>

{#snippet card(f: Entry)}
  {@const img = cover(f)}
  {@const ic = noteIcon(f)}
  <div class="wrap">
  <a class="recipe card" href={noteHash(f.path)} oncontextmenu={menuFor(f.path, false)}>
    {#if img}
      <img src={img} alt="" loading="lazy" />
    {:else}
      <div class="ph" style="background:{cardColor(f.title, f.frontmatter?.color)}"><CardIcon icon={ic} size={44} /></div>
    {/if}
    <div class="info">
      <span class="rtitle">{#if img && ic}<CardIcon icon={ic} size={16} /> {/if}{f.title}{#if f.frontmatter?.favorite === true}<span class="star" title="Favorite" aria-label="Favorite"> ★</span>{/if}</span>
      <span class="muted small">{meta(f)}</span>
      <span class="tags">
        {#if badge(f)}<span class="tag badge">{badge(f)}</span>{/if}
        {#each tagsOf(f).slice(0, 3) as t}<span class="tag">{t}</span>{/each}
        {#if f.frontmatter?.needs_review}<span class="tag warn">needs review</span>{/if}
      </span>
    </div>
  </a>
  <button class="more" aria-label="Actions for {f.title}" onclick={menuFor(f.path, false)}>⋯</button>
  </div>
{/snippet}

<div class="page">
  <header class="toolbar row">
    <Crumbs dir={path} />
    <span class="spacer"></span>
    {#if canWrite()}
      <button class="btn primary add" aria-label="Add" title="Add a recipe, files or a folder" onclick={(e) => plusMenu(e, path)}><Icon name="plus" /></button>
    {:else if app.session.pin_set}
      <button class="btn" onclick={() => (app.pinPrompt = true)}><Icon name="lock" /> Unlock editing</button>
    {/if}
    <div class="seg" role="group" aria-label="View">
      <button class="btn" class:on={view === 'cards'} onclick={() => (view = 'cards')} aria-label="Cards"><Icon name="grid" size={16} /></button>
      <button class="btn" class:on={view === 'list'} onclick={() => (view = 'list')} aria-label="List"><Icon name="list" size={16} /></button>
    </div>
  </header>

  {#if !exists && app.loaded}
    <p class="muted">This folder no longer exists. <a href="#/">Back to all recipes</a></p>
  {:else}
    {#if !path}
      <h1 class="title">Family Recipes</h1>
      {#if app.offline}<p class="muted">Offline — showing the last saved copy.</p>{/if}
    {:else}
      <h1 class="title">{nameOf(path)}</h1>
    {/if}

    {#if favorites.length}
      <h2 class="sub first">Favorites</h2>
      <div class="grid">
        {#each favorites as f (f.path)}{@render card(f)}{/each}
      </div>
      {#if subfolders.length}<h2 class="sub">Folders</h2>{/if}
    {/if}

    {#if subfolders.length}
      <div class="folders">
        {#each subfolders as f}
          <div class="wrap">
            <a class="folder card" href={folderHash(f)} style="background:{cardColor(nameOf(f), app.folderMeta[f]?.color)}" oncontextmenu={menuFor(f, true)}>
              <CardIcon icon={folderIcon(f)} size={26} />
              <span class="fname">{nameOf(f)}</span>
              <span class="count" aria-label="{countIn(f)} item{countIn(f) === 1 ? '' : 's'}">{countIn(f)}</span>
            </a>
            <button class="more" aria-label="Actions for {nameOf(f)}" onclick={menuFor(f, true)}>⋯</button>
          </div>
        {/each}
      </div>
    {/if}

    {#if files.length}
      {#if path}<h2 class="sub">{files.length} item{files.length === 1 ? '' : 's'}</h2>{/if}
      {#if view === 'cards'}
        <div class="grid">
          {#each files as f (f.path)}{@render card(f)}
          {/each}
        </div>
      {:else}
        <table class="tbl">
          <thead><tr><th>Name</th><th>Yield</th><th>Time</th><th>Tags</th><th class="no-print"></th></tr></thead>
          <tbody>
            {#each files as f (f.path)}
              <tr oncontextmenu={menuFor(f.path, false)}>
                <td><CardIcon icon={noteIcon(f)} size={16} /> <a href={noteHash(f.path)}>{f.title}</a>{#if f.frontmatter?.favorite === true}<span class="star" aria-label="Favorite"> ★</span>{/if} {#if badge(f)}<span class="tag badge">{badge(f)}</span>{/if}</td>
                <td>{f.frontmatter?.yield ?? f.frontmatter?.servings ?? ''}</td>
                <td>{f.frontmatter?.total_time ?? f.frontmatter?.cook_time ?? ''}</td>
                <td>{tagsOf(f).join(', ')}</td>
                <td class="acts no-print"><button class="btn ghost icon" aria-label="Actions for {f.title}" onclick={menuFor(f.path, false)}><Icon name="more" size={16} /></button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else if path && !subfolders.length}
      <p class="muted">This folder is empty.</p>
    {/if}
  {/if}
</div>

<style>
  .page { padding: 16px 24px 48px; max-width: 1200px; }
  .toolbar { position: sticky; top: 0; background: var(--bg); padding: 6px 0 10px; z-index: 5; }
  .seg { display: flex; }
  .seg .btn { border-radius: 0; }
  .seg .btn:first-child { border-radius: 8px 0 0 8px; }
  .seg .btn:last-child { border-radius: 0 8px 8px 0; border-left: 0; }
  .title { font-size: 2rem; margin: 8px 0 16px; }
  .sub { font-size: 1.05rem; margin: 24px 0 10px; color: var(--muted); font-family: var(--sans); font-weight: 600; }
  .folders { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 10px; }
  .wrap { position: relative; display: grid; }
  .more { position: absolute; top: 6px; right: 6px; border: 0; border-radius: 6px; background: var(--surface); color: var(--muted); padding: 0 7px; font-size: 16px; line-height: 24px; cursor: pointer; opacity: 0; box-shadow: var(--shadow); }
  .wrap:hover .more, .more:focus-visible { opacity: 1; }
  @media (hover: none) { .more { opacity: 0.85; } }
  .folder { position: relative; display: grid; gap: 2px; padding: 14px 16px; text-decoration: none; color: var(--text); border: 0; justify-items: start; }
  .folder:hover { box-shadow: var(--shadow-md); }
  .fname { font-family: var(--serif); font-size: 17px; font-weight: 600; padding-right: 22px; }
  /* item count in the corner; it gives way to the ⋯ button, which shares the corner on hover */
  .folder .count { position: absolute; top: 9px; right: 12px; font-size: 12px; color: var(--muted); font-variant-numeric: tabular-nums; transition: opacity 0.1s; }
  .wrap:hover .folder .count, .wrap:focus-within .folder .count { opacity: 0; }
  @media (hover: none) { .folder .count { right: 42px; } .wrap:hover .folder .count { opacity: 1; } }
  .star { color: #d4a017; margin-left: 4px; }
  .sub.first { margin-top: 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 14px; }
  .recipe { display: flex; flex-direction: column; text-decoration: none; color: var(--text); overflow: hidden; }
  .recipe:hover { box-shadow: var(--shadow-md); border-color: var(--accent); }
  .recipe img, .ph { width: 100%; aspect-ratio: 16 / 10; object-fit: cover; }
  .ph { display: grid; place-items: center; }
  .info { padding: 10px 12px 12px; display: grid; gap: 4px; }
  .rtitle { font-family: var(--serif); font-weight: 600; font-size: 16px; }
  .tags { display: flex; gap: 4px; flex-wrap: wrap; }
  .tag { font-size: 11px; padding: 1px 7px; border-radius: 10px; background: var(--sunken); color: var(--muted); }
  .tag.badge { background: var(--accent-soft); color: var(--accent-ink); }
  .tag.warn { background: var(--p-yellow); color: var(--text); }
  .tbl { width: 100%; border-collapse: collapse; }
  .tbl th, .tbl td { text-align: left; padding: 8px; border-bottom: 1px solid var(--border); }
  .tbl th { color: var(--muted); font-weight: 500; font-size: 13px; }
  .acts { white-space: nowrap; text-align: right; }
  @media (max-width: 899px) {
    .page { padding: 8px 14px 96px; }
    .grid { grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; }
    .toolbar .btn { min-height: 40px; }
    .toolbar .add { display: none; } /* the bottom bar has "Add" */
    .tbl th:nth-child(3), .tbl td:nth-child(3), .tbl th:nth-child(4), .tbl td:nth-child(4) { display: none; }
  }
</style>
