<script lang="ts" module>
  const PASTELS = ['--p-orange', '--p-yellow', '--p-green', '--p-blue', '--p-purple', '--p-red', '--p-brown', '--p-grey'];
  export function pastel(name: string): string {
    let h = 0;
    for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return `var(${PASTELS[h % PASTELS.length]})`;
  }
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import { app, go, noteHash, folderHash, canWrite, nameOf } from '../lib/state.svelte';
  import { fileUrl, downloadUrl } from '../lib/api';
  import { vaultTarget } from '../lib/markdown';
  import { newNote, newFolder, upload, rename, remove } from '../lib/ops';
  import { tagsOf } from '../lib/search';
  import type { Entry } from '../lib/types';

  let { path }: { path: string } = $props();
  let fileInput: HTMLInputElement | undefined = $state();
  let view = $state<'cards' | 'list'>((() => { try { return (localStorage.getItem('folderView') as any) || 'cards'; } catch { return 'cards'; } })());
  $effect(() => { try { localStorage.setItem('folderView', view); } catch {} });

  const exists = $derived(path === '' || app.folders.includes(path));
  const subfolders = $derived(app.folders.filter((f) => (path ? f.startsWith(path + '/') && !f.slice(path.length + 1).includes('/') : !f.includes('/'))));
  const files = $derived(app.files.filter((f) => (f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '') === path)
    .sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: 'base' })));
  const oldest = $derived(Math.min(...app.files.map((f) => f.mtime)));
  const recent = $derived(path ? [] : app.files.filter((f) => f.kind === 'note' && f.mtime > oldest + 60).sort((a, b) => b.mtime - a.mtime).slice(0, 8));
  const countIn = (dir: string) => app.files.filter((f) => f.path.startsWith(dir + '/')).length;
  const crumbs = $derived(path ? path.split('/').map((seg, i, a) => ({ name: seg, path: a.slice(0, i + 1).join('/') })) : []);

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

<div class="page">
  <header class="toolbar row">
    <nav class="crumbs" aria-label="Breadcrumb">
      <a href="#/">All recipes</a>
      {#each crumbs as c}<span class="sep">›</span><a href={folderHash(c.path)}>{c.name}</a>{/each}
    </nav>
    <span class="spacer"></span>
    {#if canWrite()}
      <button class="btn" onclick={() => newNote(path)}><Icon name="plus" /><span class="lbl">Recipe</span></button>
      <button class="btn" onclick={() => newFolder(path)}><Icon name="folder-plus" /><span class="lbl">Folder</span></button>
      <button class="btn" onclick={() => fileInput?.click()}><Icon name="upload" /><span class="lbl">Add files</span></button>
      <input type="file" multiple hidden bind:this={fileInput} onchange={(e) => { const t = e.currentTarget; if (t.files) upload(path, t.files); t.value = ''; }} />
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

    {#if subfolders.length}
      <div class="folders">
        {#each subfolders as f}
          <a class="folder card" href={folderHash(f)} style="background:{pastel(nameOf(f))}">
            <span class="fname">{nameOf(f)}</span>
            <span class="muted small">{countIn(f)} item{countIn(f) === 1 ? '' : 's'}</span>
          </a>
        {/each}
      </div>
    {/if}

    {#if recent.length}
      <h2 class="sub">Recently changed</h2>
      <ul class="list">
        {#each recent as f}
          <li><a href={noteHash(f.path)}>{f.title}</a> <span class="muted small">{f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : ''}</span></li>
        {/each}
      </ul>
    {/if}

    {#if files.length}
      {#if path}<h2 class="sub">{files.length} item{files.length === 1 ? '' : 's'}</h2>{/if}
      {#if view === 'cards'}
        <div class="grid">
          {#each files as f (f.path)}
            {@const img = cover(f)}
            <a class="recipe card" href={noteHash(f.path)}>
              {#if img}
                <img src={img} alt="" loading="lazy" />
              {:else}
                <div class="ph" style="background:{pastel(f.title)}">{f.title.slice(0, 1)}</div>
              {/if}
              <div class="info">
                <span class="rtitle">{f.title}</span>
                <span class="muted small">{meta(f)}</span>
                <span class="tags">
                  {#if badge(f)}<span class="tag badge">{badge(f)}</span>{/if}
                  {#each tagsOf(f).slice(0, 3) as t}<span class="tag">{t}</span>{/each}
                  {#if f.frontmatter?.needs_review}<span class="tag warn">needs review</span>{/if}
                </span>
              </div>
            </a>
          {/each}
        </div>
      {:else}
        <table class="tbl">
          <thead><tr><th>Name</th><th>Yield</th><th>Time</th><th>Tags</th><th class="no-print"></th></tr></thead>
          <tbody>
            {#each files as f (f.path)}
              <tr>
                <td><a href={noteHash(f.path)}>{f.title}</a> {#if badge(f)}<span class="tag badge">{badge(f)}</span>{/if}</td>
                <td>{f.frontmatter?.yield ?? f.frontmatter?.servings ?? ''}</td>
                <td>{f.frontmatter?.total_time ?? f.frontmatter?.cook_time ?? ''}</td>
                <td>{tagsOf(f).join(', ')}</td>
                <td class="acts no-print">
                  {#if canWrite()}
                    <button class="btn ghost small" onclick={() => rename(f.path)}>Rename</button>
                    <button class="btn ghost small" onclick={() => remove(f.path)}>Delete</button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else if path && !subfolders.length}
      <p class="muted">This folder is empty.</p>
    {/if}
    {#if path}<p class="small"><a href={downloadUrl(path)}>Download this folder as a zip</a></p>{/if}
  {/if}
</div>

<style>
  .page { padding: 16px 24px 48px; max-width: 1200px; }
  .toolbar { position: sticky; top: 0; background: var(--bg); padding: 6px 0 10px; z-index: 5; }
  .crumbs { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .crumbs a { color: var(--muted); text-decoration: none; }
  .crumbs a:last-child { color: var(--text); }
  .sep { color: var(--muted); }
  .seg { display: flex; }
  .seg .btn { border-radius: 0; }
  .seg .btn:first-child { border-radius: 8px 0 0 8px; }
  .seg .btn:last-child { border-radius: 0 8px 8px 0; border-left: 0; }
  .title { font-size: 2rem; margin: 8px 0 16px; }
  .sub { font-size: 1.05rem; margin: 24px 0 10px; color: var(--muted); font-family: var(--sans); font-weight: 600; }
  .folders { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 10px; }
  .folder { display: grid; gap: 2px; padding: 14px 16px; text-decoration: none; color: var(--text); border: 0; }
  .folder:hover { box-shadow: var(--shadow-md); }
  .fname { font-family: var(--serif); font-size: 17px; font-weight: 600; }
  .list { margin: 0; padding-left: 18px; columns: 2 260px; }
  .list li { margin: 3px 0; break-inside: avoid; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 14px; }
  .recipe { display: flex; flex-direction: column; text-decoration: none; color: var(--text); overflow: hidden; }
  .recipe:hover { box-shadow: var(--shadow-md); border-color: var(--accent); }
  .recipe img, .ph { width: 100%; aspect-ratio: 16 / 10; object-fit: cover; }
  .ph { display: grid; place-items: center; font: 600 40px var(--serif); color: rgba(0, 0, 0, 0.35); }
  :global(:root[data-theme="dark"]) .ph { color: rgba(255, 255, 255, 0.35); }
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
    .lbl { display: none; }
    .crumbs { display: none; }
    .tbl th:nth-child(3), .tbl td:nth-child(3), .tbl th:nth-child(4), .tbl td:nth-child(4) { display: none; }
  }
</style>
