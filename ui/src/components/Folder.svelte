<script lang="ts" module>
  /** the home page's pick, kept while the app is open so coming back home doesn't reshuffle it */
  let kept: { path: string; matches?: string[] | null; chosen?: boolean } | null = null;
  /** the Random card's filters, kept the same way ('' = any) */
  const filters = { tag: 'Dinner', protein: '', folder: '', fav: false };
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import { untrack, getContext } from 'svelte';
  import { app, noteHash, folderHash, canWrite, nameOf, cookbookName, toast, favorites as myFavorites, type Tab } from '../lib/state.svelte';
  import Board from './Board.svelte';
  import Today from './Today.svelte';
  import { board } from '../lib/board.svelte';
  import Crumbs from './Crumbs.svelte';
  import CardIcon from './CardIcon.svelte';
  import RecipeCard, { folderIcon, noteIcon, isFav, badge } from './RecipeCard.svelte';
  import Section from './Section.svelte';
  import { plusMenu, entryItems } from '../lib/ops';
  import { openMenu } from '../lib/menu.svelte';
  import { tagsOf } from '../lib/search';
  import { PROTEINS, blockedProteins } from '../lib/made';
  import { cardColor } from '../lib/icons';
  import type { Entry } from '../lib/types';

  let { path }: { path: string } = $props();
  const tab = getContext<Tab>('tab');
  let view = $state<'cards' | 'list'>((() => { try { return (localStorage.getItem('folderView') as any) || 'cards'; } catch { return 'cards'; } })());
  $effect(() => { try { localStorage.setItem('folderView', view); } catch {} });

  const exists = $derived(path === '' || app.folders.includes(path));
  const subfolders = $derived(app.folders.filter((f) => (path ? f.startsWith(path + '/') && !f.slice(path.length + 1).includes('/') : !f.includes('/'))));
  const files = $derived(app.files.filter((f) => (f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '') === path)
    .sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: 'base' })));
  const byTitle = (a: Entry, b: Entry) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: 'base' });
  /** everyone's bookmarks (`favorite: true`, the key kept from before) and this person's favorites */
  const bookmarks = $derived(path ? [] : app.files.filter((f) => f.kind === 'note' && f.frontmatter?.favorite === true).sort(byTitle));
  const favorites = $derived(path ? [] : myFavorites.paths.map((p) => app.byPath[p]).filter((f) => f?.kind === 'note').sort(byTitle));
  const menuFor = (p: string, folder: boolean) => (e: MouseEvent) => openMenu(e, entryItems(p, folder));
  const countIn = (dir: string) => app.files.filter((f) => f.path.startsWith(dir + '/')).length;

  // ---------- home: a suggested recipe (dinner unless the filters say otherwise) ----------
  const TAGS = ['Breakfast', 'Lunch', 'Dinner', 'Dessert', 'Snack', 'Side', 'Bread', 'Dough', 'Drink', 'Sauce'];
  const recipes = $derived(app.files.filter((f) => f.kind === 'note' && f.recipe));
  const hasTag = (f: Entry, t: string) => tagsOf(f).some((x) => x.toLowerCase() === t.toLowerCase());
  // dinners with a protein made in the last few days aren't suggested, unless that protein is chosen
  // (or nothing else is left)
  const blocked = $derived(blockedProteins(app.made));
  const avoid = (f: Entry) => hasTag(f, 'Dinner') && blocked.some((p) => hasTag(f, p));
  const allowed = (list: Entry[]) => { const ok = list.filter((f) => !avoid(f)); return ok.length ? ok : list; };
  let pickTag = $state(filters.tag);
  let pickProtein = $state(filters.protein);
  let pickFolder = $state(filters.folder);
  let pickFav = $state(filters.fav);
  let pick = $state(kept);
  /** after Random: the other matching recipes, shuffled, shown instead of Bookmarks and Folders */
  let matches = $state<string[] | null>(kept?.matches ?? null);
  const anyOf = (list: Entry[]) => (list.length ? list[Math.floor(Math.random() * list.length)].path : null);
  // shown as soon as the recipes load: a dinner recipe (any recipe while none is tagged Dinner)
  $effect(() => {
    if (path || pick || !app.loaded) return;
    const dinners = recipes.filter((f) => hasTag(f, 'Dinner'));
    const p = anyOf(allowed(dinners.length ? dinners : recipes));
    if (p) pick = kept = { path: p };
  });
  // the suggestion showing was just made (or shares its protein): show another dinner
  $effect(() => {
    if (path || !picked || pick?.chosen || !avoid(picked)) return;
    const p = anyOf(recipes.filter((f) => hasTag(f, 'Dinner') && !avoid(f)));
    if (p) untrack(() => {
      matches = matches && matches.filter((m) => app.byPath[m] && !avoid(app.byPath[m]));
      pick = kept = { path: p, matches };
    });
  });
  function randomize() {
    Object.assign(filters, { tag: pickTag, protein: pickProtein, folder: pickFolder, fav: pickFav });
    const all = recipes.filter((f) => (!pickTag || hasTag(f, pickTag)) && (!pickProtein || hasTag(f, pickProtein))
      && (!pickFolder || f.path.startsWith(pickFolder + '/')) && (!pickFav || f.frontmatter?.favorite === true));
    const pool = pickProtein ? all : allowed(all);
    if (!pool.length) return toast('No recipe matches those filters');
    for (let i = pool.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [pool[i], pool[j]] = [pool[j], pool[i]]; }
    if (pool.length > 1 && pool[0].path === pick?.path) [pool[0], pool[1]] = [pool[1], pool[0]]; // a new suggestion each time
    matches = pool.slice(1).map((f) => f.path);
    pick = kept = { path: pool[0].path, matches, chosen: !!pickProtein };
  }
  function showFolders() {
    matches = null;
    if (kept) kept.matches = null;
  }
  // tapping Home (or the logo) while on the home page brings the folders back
  let firstRoute = true;
  $effect(() => {
    tab.route;
    if (firstRoute) { firstRoute = false; return; }
    untrack(showFolders);
  });
  const matchList = $derived((matches ?? []).map((p) => app.byPath[p]).filter(Boolean));
  const picked = $derived(pick ? app.byPath[pick.path] : undefined);
</script>

{#snippet marks(f: Entry)}
  {#if f.frontmatter?.favorite === true}<span class="star" title="Bookmarked" aria-label="Bookmarked"><Icon name="bookmark" size={14} /></span>{/if}
  {#if isFav(f)}<span class="star" title="Favorite" aria-label="Favorite"><Icon name="star" size={14} /></span>{/if}
{/snippet}


<div class="page">
  <header class="toolbar row crumb-bar">
    <Crumbs dir={path} />
    <span class="bar-name">{path ? nameOf(path) : cookbookName()}{#if path && files.length}<span class="n" aria-label="{files.length} items">{files.length}</span>{/if}</span>
    <span class="spacer"></span>
    {#if !path && app.session.user}
      <button class="btn" aria-label="Write on the whiteboard" title="Write on the whiteboard" onclick={() => (board.compose = true)}><Icon name="message" /></button>
    {/if}
    {#if canWrite()}
      <button class="btn primary add" aria-label="Add" title="Add a recipe, files or a folder" onclick={(e) => plusMenu(e, path)}><Icon name="plus" /></button>
    {/if}
    <div class="seg" role="group" aria-label="View">
      <button class="btn" class:on={view === 'cards'} onclick={() => (view = 'cards')} aria-label="Cards"><Icon name="grid" size={16} /></button>
      <button class="btn" class:on={view === 'list'} onclick={() => (view = 'list')} aria-label="List"><Icon name="list" size={16} /></button>
    </div>
  </header>

  <!-- only the folder's contents scroll; the row and the folder's title above stay put -->
  {#if exists && path}<h1 class="title">{nameOf(path)}{#if files.length}<span class="n" aria-label="{files.length} items">{files.length}</span>{/if}</h1>{/if}
  <div class="scroll">
  {#if !exists && app.loaded}
    <p class="muted">This folder no longer exists. <a href="#/">Back to all recipes</a></p>
  {:else}
    {#if !path && app.offline}<p class="muted">Offline — showing the last saved copy.</p>{/if}
    {#if !path}<Board /><Today />{/if}

    {#if !path && recipes.length}
      <Section id="suggested" title="Suggested">
        <div class="card-grid suggest">
          {#if picked}<RecipeCard f={picked} />{/if}
          <div class="picker" role="group" aria-label="Random recipe">
            <label class="opt"><span>Kind</span>
              <select class="plain-select" bind:value={pickTag}><option value="">Any</option>{#each TAGS as t}<option value={t}>{t}</option>{/each}</select>
            </label>
            <label class="opt"><span>Protein</span>
              <select class="plain-select" bind:value={pickProtein}><option value="">Any</option>{#each PROTEINS as t}<option value={t}>{t}</option>{/each}</select>
            </label>
            <label class="opt"><span>Folder</span>
              <select class="plain-select" bind:value={pickFolder}><option value="">Any</option>{#each app.folders.filter((f) => !f.includes('/')) as f}<option value={f}>{f}</option>{/each}</select>
            </label>
            <label class="opt"><span>Bookmarked</span><input type="checkbox" {...{ switch: true }} bind:checked={pickFav} /></label>
            <button class="btn primary" onclick={randomize}><Icon name="shuffle" size={16} /> Random</button>
          </div>
        </div>
      </Section>
    {/if}

    {#if matches}
      <div class="row matches-head">
        <h2 class="sub">{matchList.length} more match{matchList.length === 1 ? '' : 'es'}</h2>
        <span class="spacer"></span>
        <button class="btn ghost" onclick={showFolders}>Show folders</button>
      </div>
      <div class="card-grid">
        {#each matchList as f (f.path)}<RecipeCard {f} />{/each}
      </div>
    {:else}
    {#if favorites.length}
      <Section id="favorites" title="Favorites" count={favorites.length}>
        <div class="card-grid">
          {#each favorites as f (f.path)}<RecipeCard {f} />{/each}
        </div>
      </Section>
    {/if}
    {#if bookmarks.length}
      <Section id="bookmarks" title="Bookmarks" count={bookmarks.length}>
        <div class="card-grid">
          {#each bookmarks as f (f.path)}<RecipeCard {f} />{/each}
        </div>
      </Section>
    {/if}

    {#if subfolders.length && !path && (bookmarks.length || favorites.length)}
      <Section id="folders" title="Folders" count={subfolders.length}>{@render folderCards()}</Section>
    {:else if subfolders.length}
      {@render folderCards()}
    {/if}
    {#snippet folderCards()}
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
    {/snippet}

    {#if files.length}
      {#if view === 'cards'}
        <div class="card-grid">
          {#each files as f (f.path)}<RecipeCard {f} />
          {/each}
        </div>
      {:else}
        <table class="tbl">
          <thead><tr><th>Name</th><th>Time</th><th class="no-print"></th></tr></thead>
          <tbody>
            {#each files as f (f.path)}
              <tr oncontextmenu={menuFor(f.path, false)}>
                <td class="name"><CardIcon icon={noteIcon(f)} size={16} /> <a href={noteHash(f.path)}>{f.title}</a>{@render marks(f)} {#if badge(f)}<span class="tag badge">{badge(f)}</span>{/if}</td>
                <td>{f.frontmatter?.total_time ?? f.frontmatter?.cook_time ?? ''}</td>
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
  {/if}
  </div>
</div>

<style>
  .page { display: flex; flex-direction: column; height: 100%; }
  .toolbar { flex: none; padding: 8px 24px; margin-bottom: 4px; }
  /* the row spans the column; the cards stay within 1200px, with the scrollbar at the window's edge */
  .scroll { flex: 1; min-height: 0; overflow-y: auto; padding: 0 max(24px, calc(100% - 1176px)) 48px 24px; }
  .bar-name { display: none; }
  @media print { .page { height: auto; } .scroll { overflow: visible; } }
  .seg { display: flex; }
  .seg .btn { border-radius: 0; }
  .seg .btn:first-child { border-radius: 8px 0 0 8px; }
  .seg .btn:last-child { border-radius: 0 8px 8px 0; border-left: 0; }
  .title { flex: none; font-size: 2rem; margin: 8px 24px 16px; }
  /* the folder's item count, beside its name */
  .title .n, .bar-name .n { margin-left: 0.4em; font: 500 0.55em var(--sans); color: var(--muted); font-variant-numeric: tabular-nums; vertical-align: middle; }
  .folders + .card-grid, .folders + .tbl { margin-top: 24px; }
  .folders { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 16px; }
  .wrap { position: relative; display: grid; }
  .more { position: absolute; top: 8px; right: 8px; border: 0; border-radius: 6px; background: var(--surface); color: var(--muted); padding: 0 8px; font-size: 16px; line-height: 24px; cursor: pointer; opacity: 0; box-shadow: var(--shadow); }
  .wrap:hover .more, .more:focus-visible { opacity: 1; }
  @media (hover: none) { .more { opacity: 0.85; } }
  .folder { position: relative; display: grid; gap: 2px; padding: 16px; text-decoration: none; color: var(--text); border: 0; justify-items: start; }
  .folder:hover { box-shadow: var(--shadow-md); }
  .fname { font-family: var(--serif); font-size: 17px; font-weight: 600; padding-right: 24px; }
  /* item count in the corner; it gives way to the ⋯ button, which shares the corner on hover */
  .folder .count { position: absolute; top: 8px; right: 12px; font-size: 12px; color: var(--muted); font-variant-numeric: tabular-nums; transition: opacity 0.1s; }
  .wrap:hover .folder .count, .wrap:focus-within .folder .count { opacity: 0; }
  /* on touch the ⋯ button always shows, so the count moves to the bottom corner */
  @media (hover: none) { .folder .count { top: auto; bottom: 12px; } .wrap:hover .folder .count { opacity: 1; } }
  .star { display: inline-flex; vertical-align: -2px; color: var(--accent); margin-left: 4px; }
  /* the suggested card takes the height of the Random controls beside it */
  .suggest > :global(.wrap > .recipe) { height: 100%; min-height: 186px; }
  /* beside the suggestion, on the page itself: label left, value right, hairlines between rows */
  .picker { display: flex; flex-direction: column; font-size: 14px; }
  .matches-head { margin-top: 24px; }
  .matches-head .sub { margin: 0; }
  .opt { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 40px; border-bottom: 1px solid var(--border); }
  .opt:last-of-type { border-bottom: 0; } /* no line between Bookmarked and the Random button */
  .opt span { color: var(--muted); }
  .opt select { text-align: right; text-align-last: right; }
  .opt input { accent-color: var(--accent); }
  .picker .btn { justify-content: center; margin-top: 12px; }
  /* the list view's file-type badge (cards have their own, RecipeCard.svelte) */
  .tag { font-size: 11px; padding: 2px 8px; border-radius: 10px; }
  .tag.badge { background: var(--accent-soft); color: var(--accent-ink); }
  .tbl { width: 100%; border-collapse: collapse; }
  .tbl th, .tbl td { text-align: left; padding: 8px; border-bottom: 1px solid var(--border); }
  /* a recipe's name stays on one line, cut with … when it doesn't fit */
  .tbl td.name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 0; width: 50%; }
  .tbl th { color: var(--muted); font-weight: 500; font-size: 13px; }
  .acts { white-space: nowrap; text-align: right; }
  @media (max-width: 899px) {
    .toolbar { padding: 8px 16px; flex-wrap: nowrap; }
    .scroll { padding: 0 16px 96px; }
    /* the folder's name sits in the row (back, name, add, view); the big title would repeat it */
    .bar-name { display: block; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 600 19px var(--serif); }
    .title { display: none; }
    .folders { gap: 12px; }
    .toolbar .btn { min-height: 40px; }
    .tbl td.name { width: 70%; }
  }
</style>
