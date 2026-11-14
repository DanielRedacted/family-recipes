<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { parse as parseYaml } from 'yaml';
  import Icon from './Icon.svelte';
  import Editor from './Editor.svelte';
  import Merge from './Merge.svelte';
  import Nutrition from './Nutrition.svelte';
  import { api } from '../lib/api';
  import { app, go, noteHash, toast, loadVault, canWrite } from '../lib/state.svelte';
  import { importer, enqueueUrls, enqueueFiles, enqueueText, convert, keep, retry, remove, type QueueItem, type LogStep } from '../lib/importer.svelte';
  import { renderMarkdown } from '../lib/markdown';
  import { splitRaw } from './Properties.svelte';
  import { requireWrite } from '../lib/ops';

  let busy = $state('');
  let urls = $state('');
  let pasted = $state('');
  let showPaste = $state(false);
  let dragging = $state(false);
  let fileInput: HTMLInputElement | undefined = $state();
  let urlInput: HTMLTextAreaElement | undefined = $state();
  let pasteBox: HTMLTextAreaElement | undefined = $state();
  let folder = $state((() => { try { return localStorage.getItem('importFolder') ?? ''; } catch { return ''; } })());
  $effect(() => { try { localStorage.setItem('importFolder', folder); } catch {} });
  $effect(() => { if (importer.folder !== null) { folder = importer.folder; importer.folder = null; } });

  let tab = $state<'edit' | 'preview' | 'source'>('edit');
  let checkTimer: ReturnType<typeof setTimeout> | undefined;
  let aiBusy = $state(false);
  let aiProposal = $state<string | null>(null);
  let openLog = $state<Record<number, boolean>>({});

  const folders = $derived(['', ...app.folders.filter((f) => f !== 'attachments' && !f.startsWith('attachments/'))]);
  const cur = $derived(importer.queue.find((q) => q.id === importer.selected && q.status === 'ready' && q.review && q.job));
  const others = $derived(importer.queue.filter((q) => q !== cur));

  // the "+" menu asks for a particular input
  $effect(() => {
    const f = importer.focus;
    if (!f) return;
    importer.focus = null;
    importer.selected = null;
    tick().then(() => {
      if (f === 'url') urlInput?.focus();
      else if (f === 'file') fileInput?.click();
      else { showPaste = true; tick().then(() => pasteBox?.focus()); }
    });
  });

  function fromUrls(e?: Event) {
    e?.preventDefault();
    if (!urls.trim() || !requireWrite()) return;
    const n = enqueueUrls(urls);
    if (!n) toast('No new web address found (they start with https://)', 'error');
    else urls = '';
  }

  function pick(files: FileList | File[] | null | undefined) {
    if (!files?.length || !requireWrite()) return;
    enqueueFiles(files);
  }

  function usePasted() {
    if (!pasted.trim() || !requireWrite()) return;
    enqueueText(pasted);
    pasted = '';
    showPaste = false;
  }

  async function keepItem(it: QueueItem) {
    busy = 'Saving…';
    try {
      const path = await keep(it.id, folder);
      await loadVault();
      toast('Saved — the original file is kept as is', 'ok');
      if (!importer.queue.length) go(noteHash(path));
    } catch (err: any) { toast(err.message, 'error'); }
    finally { busy = ''; }
  }

  function onEdit(v: string) {
    if (!cur?.review) return;
    const rv = cur.review;
    rv.md = v;
    clearTimeout(checkTimer);
    checkTimer = setTimeout(async () => {
      try { rv.issues = (await api('POST', '/api/import/check', { markdown: rv.md })).issues; } catch {}
    }, 700);
  }

  async function save() {
    if (!cur?.review || !cur.job) return;
    const { review: rv, job, id } = cur;
    busy = 'Saving…';
    try {
      const images = Object.keys(rv.chosen).map(Number).filter((i) => rv.chosen[i] && !rv.hidden[i]);
      const r = await api('POST', `/api/import/${job.job}/save`, { folder, name: rv.name, markdown: rv.md, images, cover: rv.cover !== null && rv.chosen[rv.cover] ? rv.cover : null });
      remove(id, false);
      await loadVault();
      tab = 'edit';
      if (importer.queue.length) toast(`Saved “${rv.name}”`, 'ok');
      else { toast('Recipe saved', 'ok'); go(noteHash(r.path)); }
    } catch (err: any) { toast(err.message, 'error'); }
    finally { busy = ''; }
  }

  function discard() {
    if (cur) remove(cur.id);
    tab = 'edit';
  }

  async function fixWithAi() {
    if (!cur?.review || !cur.job) return;
    aiBusy = true;
    try { aiProposal = (await api('POST', '/api/ai/fix', { markdown: cur.review.md, source_text: cur.job.source_text })).markdown; }
    catch (err: any) { toast(err.message, 'error'); }
    finally { aiBusy = false; }
  }

  function imgLoaded(e: Event, i: number) {
    const im = e.currentTarget as HTMLImageElement;
    if (cur?.review && (im.naturalWidth < 200 || im.naturalHeight < 150)) cur.review.hidden[i] = true;
  }

  function statusText(it: QueueItem): string {
    switch (it.status) {
      case 'choose': return 'Keep as is, or convert?';
      case 'waiting': return 'Waiting';
      case 'running': return it.log.at(-1)?.detail ?? 'Starting…';
      case 'ready': return 'Ready to review';
      case 'failed': return it.error ?? 'Failed';
    }
  }

  onDestroy(() => clearTimeout(checkTimer));
  const preview = $derived(cur?.review && tab === 'preview' ? renderMarkdown(splitRaw(cur.review.md)[1], folder + '/x.md') : '');
  const nutrition = $derived.by(() => {
    if (!cur?.review) return null;
    try { return (parseYaml(splitRaw(cur.review.md)[0] || '{}') as any)?.nutrition ?? null; } catch { return null; }
  });
  const warnCount = $derived(cur?.review ? cur.review.issues.filter((i) => i.level === 'warn').length : 0);
  const visibleImages = $derived(cur?.job && cur.review ? cur.job.images.filter((c) => !cur.review!.hidden[c.i]) : []);
  const methodNote: Record<string, string> = {
    'wprm-rest': 'Read from the site’s own recipe data (WP Recipe Maker).',
    'json-ld': 'Read from the recipe data in the page (schema.org).',
    wprm: 'Read from the recipe card on the page (WP Recipe Maker).',
    microdata: 'Read from the recipe data in the page (schema.org microdata).',
    page: 'This page had no recipe data, so this is its main text — please check it.',
    document: 'Converted from the document.',
  };
</script>

{#snippet logList(log: LogStep[])}
  <ol class="log">
    {#each log as s}<li class:bad={!s.ok}><code>{s.step}</code> {s.detail} <span class="muted">{s.ms} ms</span></li>{/each}
  </ol>
{/snippet}

<div class="page">
  {#if !canWrite()}
    <h1>Add recipes</h1>
    <p class="muted">Importing needs editing rights. {#if app.session.pin_set}<button class="btn" onclick={() => (app.pinPrompt = true)}>Unlock editing</button>{/if}</p>
  {:else if cur && cur.review && cur.job}
    {@const rv = cur.review}
    {@const job = cur.job}
    <header class="rev row">
      <button class="btn ghost" onclick={() => (importer.selected = null)} aria-label="All imports">‹ {importer.queue.length > 1 ? `Imports (${importer.queue.length})` : 'Back'}</button>
      <input class="field title" bind:value={rv.name} aria-label="Recipe name" />
      <select class="field w" bind:value={folder} aria-label="Folder">{#each folders as f}<option value={f}>{f || 'Top level'}</option>{/each}</select>
      <span class="spacer"></span>
      {#if app.session.ai}<button class="btn" disabled={aiBusy} onclick={fixWithAi}>{aiBusy ? 'Asking AI…' : '✨ Fix with AI'}</button>{/if}
      <button class="btn" onclick={discard}>Discard</button>
      <button class="btn primary" disabled={!!busy || !rv.name.trim()} onclick={save}>Save recipe</button>
    </header>

    {#if job.duplicate}
      <div class="banner">You may already have this recipe: <a href={noteHash(job.duplicate)} target="_blank">{job.duplicate}</a></div>
    {/if}
    <p class="muted small">
      {methodNote[job.method] ?? ''}
      {#if job.original} The original file ({job.original}) is kept with the recipe.{/if}
    </p>

    {#key cur.id}
    <div class="grid">
      <div class="main">
        <div class="tabs row" role="tablist">
          <button role="tab" aria-selected={tab === 'edit'} class="btn" class:on={tab === 'edit'} onclick={() => (tab = 'edit')}>Edit</button>
          <button role="tab" aria-selected={tab === 'preview'} class="btn" class:on={tab === 'preview'} onclick={() => (tab = 'preview')}>Preview</button>
          {#if job.source_text}<button role="tab" aria-selected={tab === 'source'} class="btn" class:on={tab === 'source'} onclick={() => (tab = 'source')}>Original text</button>{/if}
        </div>
        <div class="pane" class:hide={tab !== 'edit'}><Editor value={rv.md} onchange={onEdit} notePath={folder + '/x.md'} /></div>
        {#if tab === 'preview'}<div class="pane prose scroll">{@html preview}<Nutrition data={nutrition} /></div>{/if}
        {#if tab === 'source'}<pre class="pane scroll src">{job.source_text}</pre>{/if}
      </div>

      <aside class="side">
        <section class="card block">
          <h3>{warnCount ? `${warnCount} thing${warnCount > 1 ? 's' : ''} to check` : 'Looks good'}</h3>
          {#if rv.issues.length}
            <ul class="issues">{#each rv.issues as it}<li class={it.level}>{it.message}</li>{/each}</ul>
            <p class="muted small">Suggestions only — you can save as is and tidy later.</p>
          {:else}
            <p class="muted small">Title, ingredients and steps were all found.</p>
          {/if}
        </section>

        {#if job.images.length}
          <section class="card block">
            <h3>Pictures <span class="muted small">— tick the ones to keep</span></h3>
            <div class="imgs">
              {#each job.images as c (c.i)}
                {#if !rv.hidden[c.i]}
                  <figure class:sel={rv.chosen[c.i]}>
                    <label>
                      <img src="/api/import/{job.job}/image/{c.i}" alt={c.alt} loading="lazy" onload={(e) => imgLoaded(e, c.i)} onerror={() => (rv.hidden[c.i] = true)} />
                      <span class="tick"><input type="checkbox" bind:checked={rv.chosen[c.i]} aria-label="Keep this picture" /></span>
                    </label>
                    <figcaption class="row">
                      <label class="small"><input type="radio" name="cover" checked={rv.cover === c.i} onchange={() => { rv.cover = c.i; rv.chosen[c.i] = true; }} /> Cover</label>
                      {#if c.kind === 'step'}<span class="muted small">step</span>{/if}
                    </figcaption>
                  </figure>
                {/if}
              {/each}
            </div>
            {#if !visibleImages.length}<p class="muted small">No usable pictures found.</p>{/if}
          </section>
        {/if}

        <details class="card block" open={job.method === 'page'}>
          <summary><h3>Import details</h3></summary>
          {@render logList(cur.log)}
        </details>
      </aside>
    </div>
    {/key}
  {:else}
    <h1>Add recipes</h1>
    <label class="row dest">Save into
      <select class="field w" bind:value={folder}>{#each folders as f}<option value={f}>{f || 'Top level'}</option>{/each}</select>
    </label>

    {#if importer.queue.length}
      <section class="card block queue" aria-label="Imports">
        <h2>Imports</h2>
        <ul>
          {#each importer.queue as it (it.id)}
            <li class="qi {it.status}">
              <div class="row">
                <span class="dot" aria-hidden="true"></span>
                <span class="label" title={it.label}>{it.label}</span>
                <span class="spacer"></span>
                {#if it.status === 'ready'}<button class="btn primary small" onclick={() => (importer.selected = it.id)}>Review</button>{/if}
                {#if it.status === 'failed'}<button class="btn small" onclick={() => retry(it.id)}>Try again</button>{/if}
                {#if it.log.length}<button class="btn ghost small" onclick={() => (openLog[it.id] = !openLog[it.id])}>{openLog[it.id] ? 'Hide details' : 'Details'}</button>{/if}
                <button class="btn ghost icon" aria-label="Remove" onclick={() => remove(it.id)}><Icon name="x" size={16} /></button>
              </div>
              <p class="small status">{statusText(it)}</p>
              {#if it.status === 'choose'}
                {@const pdf = /\.pdf$/i.test(it.label)}
                <div class="choices">
                  <div class="choice">
                    <strong>Keep as {pdf ? 'a PDF' : 'a photo'}</strong> <span class="tag">recommended</span>
                    <p class="muted small">Saved exactly as it is. Searchable by title and tags; you can convert it later.</p>
                    <div class="row">
                      <input class="field" bind:value={it.keepTitle} aria-label="Title" placeholder="Title" />
                      <input class="field" bind:value={it.keepTags} aria-label="Tags" placeholder="Tags (optional, comma separated)" />
                      <button class="btn primary" disabled={!!busy || !it.keepTitle?.trim()} onclick={() => keepItem(it)}>Keep</button>
                    </div>
                  </div>
                  {#if pdf}
                    <div class="choice">
                      <strong>Convert to an editable recipe</strong>
                      <p class="muted small">Reads the text out of the PDF into a recipe you can edit, scale and cook from.{app.session.ai ? '' : ' Without AI the layout usually needs tidying.'} The PDF is kept too.</p>
                      <button class="btn" onclick={() => convert(it.id)}>Convert</button>
                    </div>
                  {/if}
                </div>
              {/if}
              {#if openLog[it.id] || it.status === 'failed'}{@render logList(it.log)}{/if}
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section class="card block">
      <h2>From a website</h2>
      <form class="grid1" onsubmit={fromUrls}>
        <textarea class="field" rows="2" bind:this={urlInput} bind:value={urls} placeholder="https://… recipe page address (paste several, one per line)" aria-label="Recipe web addresses"
          onkeydown={(e) => { if (e.key === 'Enter' && !e.shiftKey) fromUrls(e); }}></textarea>
        <div><button class="btn primary" disabled={!urls.trim()}>Get recipe</button></div>
      </form>
      <p class="muted small">Recipe sites usually include the recipe in a standard format (schema.org, WP Recipe Maker), which is used when present. Otherwise you can tidy the page text before saving.</p>
    </section>

    <section
      class="card block drop"
      class:dragging
      aria-label="Drop files"
      ondragover={(e) => { e.preventDefault(); dragging = true; }}
      ondragleave={() => (dragging = false)}
      ondrop={(e) => { e.preventDefault(); dragging = false; pick(e.dataTransfer?.files); }}
    >
      <h2>From a file</h2>
      <p>Drop PDFs, Word documents (.docx, .doc), photos, saved web pages or text files here — or</p>
      <div><button class="btn" onclick={() => fileInput?.click()}><Icon name="upload" /> Choose files</button></div>
      <input type="file" hidden multiple bind:this={fileInput} accept=".pdf,.docx,.doc,.rtf,.odt,.html,.htm,.txt,.md,image/*" onchange={(e) => { pick(e.currentTarget.files); e.currentTarget.value = ''; }} />
      <p class="muted small">The original file is always kept with the recipe.</p>
    </section>

    <section class="card block">
      <button class="btn ghost" onclick={() => (showPaste = !showPaste)}>{showPaste ? '▾' : '▸'} Paste recipe text</button>
      {#if showPaste}
        <textarea class="field" rows="10" bind:this={pasteBox} bind:value={pasted} placeholder="Paste the recipe here…"></textarea>
        <div><button class="btn primary" disabled={!pasted.trim()} onclick={usePasted}>Use this text</button></div>
      {/if}
    </section>
  {/if}

  {#if busy}<div class="busy" role="status">{busy}</div>{/if}
</div>

{#if aiProposal !== null && cur?.review}
  <Merge theirs={cur.review.md} mine={aiProposal} leftLabel="Current draft" rightLabel="AI suggestion (edit freely)" accept="Use suggestion"
    onaccept={(t) => { onEdit(t); aiProposal = null; }} oncancel={() => (aiProposal = null)} />
{/if}

<style>
  .page { padding: 16px 28px 80px; }
  h1 { margin: 6px 0 14px; }
  h2 { margin: 0 0 8px; font-size: 1.1rem; }
  h3 { margin: 0 0 8px; font-size: 1rem; font-family: var(--sans); display: inline; }
  .block { padding: 16px 18px; margin-bottom: 14px; display: grid; gap: 10px; max-width: 820px; }
  .block p { margin: 0; }
  .grid1 { display: grid; gap: 8px; }
  .w { width: auto; }
  .dest { margin-bottom: 14px; }
  .drop.dragging { outline: 2px dashed var(--accent); background: var(--accent-soft); }
  .queue ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .qi { border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px; background: var(--bg); }
  .qi .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; max-width: 60%; font-weight: 500; }
  .qi .status { margin: 2px 0 0 18px; color: var(--muted); }
  .qi.failed .status { color: var(--danger); }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--muted); flex: none; }
  .qi.running .dot { background: var(--accent); animation: pulse 1s infinite alternate; }
  .qi.ready .dot { background: var(--ok); }
  .qi.failed .dot { background: var(--danger); }
  .qi.choose .dot { background: var(--accent-ink); }
  @keyframes pulse { to { opacity: 0.3; } }
  .choices { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 10px; margin: 8px 0 2px 18px; }
  .choice { display: grid; gap: 6px; align-content: start; }
  .choice .field { flex: 1; min-width: 120px; }
  .tag { font-size: 11px; padding: 1px 7px; border-radius: 10px; background: var(--accent-soft); color: var(--accent-ink); font-family: var(--sans); }
  .log { margin: 6px 0 0 18px; padding-left: 16px; font-size: 13px; display: grid; gap: 3px; }
  .log code { font-size: 12px; background: var(--sunken); padding: 0 4px; border-radius: 4px; }
  .log li.bad { color: var(--muted); }
  .log li.bad::marker { color: var(--danger); }
  details summary { cursor: pointer; }
  .rev { position: sticky; top: 0; background: var(--bg); z-index: 5; padding: 6px 0 10px; }
  .title { flex: 1; min-width: 200px; font: 600 18px var(--serif); }
  .banner { background: var(--p-yellow); padding: 8px 12px; border-radius: 8px; margin-bottom: 8px; }
  .grid { display: grid; grid-template-columns: minmax(0, 1fr) 340px; gap: 18px; align-items: start; }
  .tabs { margin-bottom: 8px; gap: 4px; }
  .pane { height: calc(100vh - 230px); min-height: 360px; }
  .hide { display: none; }
  .scroll { overflow: auto; border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 18px; background: var(--surface); }
  .src { white-space: pre-wrap; font: 13px/1.5 var(--mono); margin: 0; }
  .side { display: grid; gap: 0; position: sticky; top: 64px; max-height: calc(100vh - 90px); overflow: auto; }
  .side .block { max-width: none; }
  .side .log { margin-left: 0; }
  .issues { margin: 0; padding-left: 18px; font-size: 14px; display: grid; gap: 6px; }
  .issues li.warn::marker { color: var(--danger); }
  .issues li.info { color: var(--muted); }
  .imgs { display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; }
  figure { margin: 0; border: 2px solid transparent; border-radius: 8px; overflow: hidden; background: var(--sunken); }
  figure.sel { border-color: var(--accent); }
  figure label { display: block; position: relative; cursor: pointer; }
  figure img { width: 100%; aspect-ratio: 4 / 3; object-fit: cover; display: block; }
  .tick { position: absolute; top: 6px; left: 6px; background: rgba(255, 255, 255, 0.85); border-radius: 4px; padding: 2px 4px; }
  .tick input { width: 18px; height: 18px; accent-color: var(--accent); }
  figcaption { padding: 4px 6px; justify-content: space-between; }
  .busy { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.25); display: grid; place-items: center; color: #fff; font-size: 18px; z-index: 70; }
  @media (max-width: 999px) {
    .grid { grid-template-columns: 1fr; }
    .side { position: static; max-height: none; }
    .pane { height: 60vh; }
  }
  @media (max-width: 899px) { .page { padding: 8px 14px 96px; } .qi .label { max-width: 45%; } }
</style>
