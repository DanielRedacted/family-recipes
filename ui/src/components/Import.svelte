<script lang="ts">
  import { onDestroy } from 'svelte';
  import Icon from './Icon.svelte';
  import Editor from './Editor.svelte';
  import Merge from './Merge.svelte';
  import { api } from '../lib/api';
  import { app, go, noteHash, toast, loadVault, canWrite } from '../lib/state.svelte';
  import { importer, importFile, type ImportJob, type Issue } from '../lib/importer.svelte';
  import { renderMarkdown } from '../lib/markdown';
  import { splitRaw } from './Properties.svelte';
  import { requireWrite } from '../lib/ops';

  type Step = 'start' | 'choose' | 'review';
  let step = $state<Step>('start');
  let busy = $state('');
  let url = $state('');
  let pasted = $state('');
  let showPaste = $state(false);
  let dragging = $state(false);
  let fileInput: HTMLInputElement | undefined = $state();
  let folder = $state((() => { try { return localStorage.getItem('importFolder') ?? ''; } catch { return ''; } })());
  $effect(() => { try { localStorage.setItem('importFolder', folder); } catch {} });

  // PDF / photo choice
  let pending = $state<File | null>(null);
  let keepTitle = $state('');
  let keepTags = $state('');

  // review
  let job = $state<ImportJob | null>(null);
  let md = $state('');
  let name = $state('');
  let issues = $state<Issue[]>([]);
  let tab = $state<'edit' | 'preview' | 'source'>('edit');
  let chosen = $state<Record<number, boolean>>({});
  let cover = $state<number | null>(null);
  let hidden = $state<Record<number, boolean>>({});
  let checkTimer: ReturnType<typeof setTimeout> | undefined;
  let aiBusy = $state(false);

  const folders = $derived(['', ...app.folders.filter((f) => f !== 'attachments' && !f.startsWith('attachments/'))]);

  $effect(() => {
    if (importer.pending) { open(importer.pending); importer.pending = null; }
  });

  function open(j: ImportJob) {
    job = j;
    md = j.markdown;
    name = j.title || 'Imported recipe';
    issues = j.issues;
    chosen = {};
    hidden = {};
    const first = j.images.find((c) => c.kind === 'recipe' || c.kind === 'document') ?? null;
    cover = first?.i ?? null;
    if (first) chosen[first.i] = true;
    tab = 'edit';
    step = 'review';
  }

  async function fromUrl(e?: Event) {
    e?.preventDefault();
    if (!url.trim() || !requireWrite()) return;
    busy = 'Reading the page…';
    try { open(await api('POST', '/api/import/url', { url: url.trim() })); }
    catch (err: any) { toast(err.message, 'error'); }
    finally { busy = ''; }
  }

  async function ingest(file: File) {
    busy = /\.pdf$/i.test(file.name) ? 'Reading the PDF…' : 'Converting the document…';
    try { open(await importFile(file, 'ingest')); }
    catch (err: any) { toast(err.message, 'error'); }
    finally { busy = ''; }
  }

  function pick(files: FileList | File[] | null | undefined) {
    const file = files?.[0];
    if (!file || !requireWrite()) return;
    if (/\.pdf$/i.test(file.name) || file.type.startsWith('image/')) {
      pending = file;
      keepTitle = file.name.replace(/\.[^.]+$/, '');
      keepTags = '';
      step = 'choose';
    } else ingest(file);
  }

  async function keep() {
    if (!pending) return;
    busy = 'Saving…';
    try {
      const r = await importFile(pending, 'keep', { title: keepTitle, folder, tags: keepTags });
      await loadVault();
      toast('Saved — the original file is kept as is', 'ok');
      go(noteHash(r.path));
    } catch (err: any) { toast(err.message, 'error'); }
    finally { busy = ''; }
  }

  function usePasted() {
    if (!pasted.trim() || !requireWrite()) return;
    ingest(new File([pasted], 'pasted.txt', { type: 'text/plain' }));
  }

  function onEdit(v: string) {
    md = v;
    clearTimeout(checkTimer);
    checkTimer = setTimeout(async () => {
      try { issues = (await api('POST', '/api/import/check', { markdown: md })).issues; } catch {}
    }, 700);
  }

  async function save() {
    if (!job) return;
    busy = 'Saving…';
    try {
      const images = Object.keys(chosen).map(Number).filter((i) => chosen[i] && !hidden[i]);
      const r = await api('POST', `/api/import/${job.job}/save`, { folder, name, markdown: md, images, cover: cover !== null && chosen[cover] ? cover : null });
      await loadVault();
      job = null;
      step = 'start';
      url = '';
      toast('Recipe saved', 'ok');
      go(noteHash(r.path));
    } catch (err: any) { toast(err.message, 'error'); }
    finally { busy = ''; }
  }

  async function cancel() {
    if (job) api('DELETE', `/api/import/${job.job}`).catch(() => {});
    job = null;
    pending = null;
    step = 'start';
  }

  async function fixWithAi() {
    if (!job) return;
    aiBusy = true;
    try {
      const r = await api('POST', '/api/ai/fix', { markdown: md, source_text: job.source_text });
      aiProposal = r.markdown;
    } catch (err: any) { toast(err.message, 'error'); }
    finally { aiBusy = false; }
  }
  let aiProposal = $state<string | null>(null);

  function imgLoaded(e: Event, i: number) {
    const im = e.currentTarget as HTMLImageElement;
    if (im.naturalWidth < 200 || im.naturalHeight < 150) hidden[i] = true;
  }

  onDestroy(() => clearTimeout(checkTimer));
  const preview = $derived(step === 'review' && tab === 'preview' ? renderMarkdown(splitRaw(md)[1], folder + '/x.md') : '');
  const isPdf = $derived(/\.pdf$/i.test(pending?.name ?? ''));
  const warnCount = $derived(issues.filter((i) => i.level === 'warn').length);
  const visibleImages = $derived(job ? job.images.filter((c) => !hidden[c.i]) : []);
</script>

<div class="page">
  {#if !canWrite()}
    <h1>Import a recipe</h1>
    <p class="muted">Importing needs editing rights. {#if app.session.pin_set}<button class="btn" onclick={() => (app.pinPrompt = true)}>Unlock editing</button>{/if}</p>
  {:else if step === 'start'}
    <h1>Import a recipe</h1>
    <label class="row dest">Save into
      <select class="field w" bind:value={folder}>{#each folders as f}<option value={f}>{f || 'Top level'}</option>{/each}</select>
    </label>

    <section class="card block">
      <h2>From a website</h2>
      <form class="row" onsubmit={fromUrl}>
        <input class="field grow" type="url" placeholder="https://… recipe page address" bind:value={url} aria-label="Recipe web address" />
        <button class="btn primary" disabled={!url.trim() || !!busy}>Get recipe</button>
      </form>
      <p class="muted small">Recipe sites usually include the recipe in a standard format, which is used when present. Otherwise you can tidy the page text before saving.</p>
    </section>

    <section
      class="card block drop"
      class:dragging
      aria-label="Drop a file"
      ondragover={(e) => { e.preventDefault(); dragging = true; }}
      ondragleave={() => (dragging = false)}
      ondrop={(e) => { e.preventDefault(); dragging = false; pick(e.dataTransfer?.files); }}
    >
      <h2>From a file</h2>
      <p>Drop a PDF, Word document (.docx, .doc), photo, saved web page, or text file here — or</p>
      <button class="btn" onclick={() => fileInput?.click()}><Icon name="upload" /> Choose a file</button>
      <input type="file" hidden bind:this={fileInput} accept=".pdf,.docx,.doc,.rtf,.odt,.html,.htm,.txt,.md,image/*" onchange={(e) => { pick(e.currentTarget.files); e.currentTarget.value = ''; }} />
      <p class="muted small">The original file is always kept with the recipe.</p>
    </section>

    <section class="card block">
      <button class="btn ghost" onclick={() => (showPaste = !showPaste)}>{showPaste ? '▾' : '▸'} Paste recipe text</button>
      {#if showPaste}
        <textarea class="field" rows="10" bind:value={pasted} placeholder="Paste the recipe here…"></textarea>
        <button class="btn primary" disabled={!pasted.trim()} onclick={usePasted}>Use this text</button>
      {/if}
    </section>
  {:else if step === 'choose' && pending}
    <h1>{pending.name}</h1>
    <div class="choices">
      <section class="card block choice">
        <h2>Keep as {isPdf ? 'a PDF' : 'a photo'} <span class="tag">recommended</span></h2>
        <p class="muted small">Saved exactly as it is and shown as-is. You can search for it by title and tags, and convert it later if you like.</p>
        <label>Title <input class="field" bind:value={keepTitle} /></label>
        <label>Folder <select class="field" bind:value={folder}>{#each folders as f}<option value={f}>{f || 'Top level'}</option>{/each}</select></label>
        <label>Tags <input class="field" bind:value={keepTags} placeholder="optional, comma separated" /></label>
        <button class="btn primary" disabled={!!busy || !keepTitle.trim()} onclick={keep}>Save</button>
      </section>
      {#if isPdf}
        <section class="card block choice">
          <h2>Convert to an editable recipe</h2>
          <p class="muted small">Reads the text out of the PDF into a recipe you can edit, scale and cook from.{app.session.ai ? '' : ' Without AI the layout usually needs tidying by hand.'} The PDF is kept too.</p>
          <button class="btn" disabled={!!busy} onclick={() => pending && ingest(pending)}>Convert</button>
        </section>
      {/if}
    </div>
    <button class="btn ghost" onclick={cancel}>Cancel</button>
  {:else if step === 'review' && job}
    <header class="rev row">
      <input class="field title" bind:value={name} aria-label="Recipe name" />
      <select class="field w" bind:value={folder} aria-label="Folder">{#each folders as f}<option value={f}>{f || 'Top level'}</option>{/each}</select>
      <span class="spacer"></span>
      {#if app.session.ai}<button class="btn" disabled={aiBusy} onclick={fixWithAi}>{aiBusy ? 'Asking AI…' : '✨ Fix with AI'}</button>{/if}
      <button class="btn" onclick={cancel}>Cancel</button>
      <button class="btn primary" disabled={!!busy || !name.trim()} onclick={save}>Save recipe</button>
    </header>

    {#if job.duplicate}
      <div class="banner">You may already have this recipe: <a href={noteHash(job.duplicate)} target="_blank">{job.duplicate}</a></div>
    {/if}
    <p class="muted small">
      {#if job.method === 'json-ld' || job.method === 'wprm' || job.method === 'microdata'}Read from the recipe data in the page.
      {:else if job.method === 'page'}This page had no recipe data, so this is its main text — please check it.
      {:else}Converted from the document.{/if}
      {#if job.original} The original file ({job.original}) is kept with the recipe.{/if}
    </p>

    <div class="grid">
      <div class="main">
        <div class="tabs row" role="tablist">
          <button role="tab" aria-selected={tab === 'edit'} class="btn" class:on={tab === 'edit'} onclick={() => (tab = 'edit')}>Edit</button>
          <button role="tab" aria-selected={tab === 'preview'} class="btn" class:on={tab === 'preview'} onclick={() => (tab = 'preview')}>Preview</button>
          {#if job.source_text}<button role="tab" aria-selected={tab === 'source'} class="btn" class:on={tab === 'source'} onclick={() => (tab = 'source')}>Original text</button>{/if}
        </div>
        <div class="pane" class:hide={tab !== 'edit'}><Editor value={md} onchange={onEdit} /></div>
        {#if tab === 'preview'}<div class="pane prose scroll">{@html preview}</div>{/if}
        {#if tab === 'source'}<pre class="pane scroll src">{job.source_text}</pre>{/if}
      </div>

      <aside class="side">
        <section class="card block">
          <h3>{warnCount ? `${warnCount} thing${warnCount > 1 ? 's' : ''} to check` : 'Looks good'}</h3>
          {#if issues.length}
            <ul class="issues">{#each issues as it}<li class={it.level}>{it.message}</li>{/each}</ul>
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
                {#if !hidden[c.i]}
                  <figure class:sel={chosen[c.i]}>
                    <label>
                      <img src="/api/import/{job.job}/image/{c.i}" alt={c.alt} loading="lazy" onload={(e) => imgLoaded(e, c.i)} onerror={() => (hidden[c.i] = true)} />
                      <span class="tick"><input type="checkbox" bind:checked={chosen[c.i]} aria-label="Keep this picture" /></span>
                    </label>
                    <figcaption class="row">
                      <label class="small"><input type="radio" name="cover" checked={cover === c.i} onchange={() => { cover = c.i; chosen[c.i] = true; }} /> Cover</label>
                      {#if c.kind === 'step'}<span class="muted small">step</span>{/if}
                    </figcaption>
                  </figure>
                {/if}
              {/each}
            </div>
            {#if !visibleImages.length}<p class="muted small">No usable pictures found.</p>{/if}
          </section>
        {/if}
      </aside>
    </div>
  {/if}

  {#if busy}<div class="busy" role="status">{busy}</div>{/if}
</div>

{#if aiProposal !== null}
  <Merge theirs={md} mine={aiProposal} leftLabel="Current draft" rightLabel="AI suggestion (edit freely)" accept="Use suggestion"
    onaccept={(t) => { md = t; onEdit(t); aiProposal = null; }} oncancel={() => (aiProposal = null)} />
{/if}

<style>
  .page { padding: 16px 28px 80px; }
  h1 { margin: 6px 0 14px; }
  h2 { margin: 0 0 8px; font-size: 1.1rem; }
  h3 { margin: 0 0 8px; font-size: 1rem; font-family: var(--sans); }
  .block { padding: 16px 18px; margin-bottom: 14px; display: grid; gap: 10px; max-width: 820px; }
  .block p { margin: 0; }
  .grow { flex: 1; min-width: 220px; }
  .w { width: auto; }
  .dest { margin-bottom: 14px; }
  .drop.dragging { outline: 2px dashed var(--accent); background: var(--accent-soft); }
  .choices { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 14px; max-width: 900px; }
  .choice label { display: grid; gap: 4px; font-size: 14px; }
  .tag { font-size: 11px; padding: 1px 7px; border-radius: 10px; background: var(--accent-soft); color: var(--accent-ink); font-family: var(--sans); }
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
  @media (max-width: 899px) { .page { padding: 8px 14px 96px; } }
</style>
