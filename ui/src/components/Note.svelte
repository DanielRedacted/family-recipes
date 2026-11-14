<script lang="ts">
  import Icon from './Icon.svelte';
  import { untrack } from 'svelte';
  import { app, canWrite, go, folderHash, noteHash, dirOf, nameOf, toast, loadVault } from '../lib/state.svelte';
  import { fileUrl, saveFile, ApiError, downloadUrl } from '../lib/api';
  import { renderMarkdown, scaledRaw, bodyOf } from '../lib/markdown';
  import { yieldNumber, type Units } from '../lib/scale';
  import { openMenu } from '../lib/menu.svelte';
  import { rename, moveDialog, remove, requireWrite, attachImage, shell } from '../lib/ops';
  import Editor from './Editor.svelte';
  import Merge from './Merge.svelte';
  import Properties, { setProp } from './Properties.svelte';
  import CookMode from './CookMode.svelte';
  import PdfView from './PdfView.svelte';
  import Nutrition from './Nutrition.svelte';
  import { enqueueStarted } from '../lib/importer.svelte';
  import { api } from '../lib/api';
  import { vaultTarget } from '../lib/markdown';

  let { path, mode: routeMode }: { path: string; mode?: string } = $props();

  const entry = $derived(app.byPath[path]);
  const wide = typeof matchMedia !== 'undefined' && matchMedia('(min-width: 1100px)').matches;
  let mode = $state<'read' | 'edit' | 'split'>('read');
  let draft = $state('');
  let base = $state<string | null>(null); // etag of the text the draft started from
  let dirty = $state(false);
  let saving = $state(false);
  let conflict = $state<{ theirs: string; etag: string | null } | null>(null);
  let comparing = $state(false);
  let cooking = $state(false);
  let factor = $state(1);
  let units = $state<Units>((() => { try { return (localStorage.getItem('units') as Units) || 'original'; } catch { return 'original'; } })());
  let timer: ReturnType<typeof setTimeout> | undefined;
  let editor: Editor | undefined = $state();
  let imgInput: HTMLInputElement | undefined = $state();

  // reset when switching notes
  $effect(() => {
    path;
    untrack(() => {
      flushSave();
      mode = routeMode === 'edit' ? (wide ? 'split' : 'edit') : 'read';
      const e = app.byPath[path];
      draft = e?.raw ?? '';
      base = e?.etag ?? null;
      dirty = false;
      conflict = null;
      factor = 1;
    });
  });

  // the file changed on disk (another device, another editor, or our own save)
  $effect(() => {
    const e = entry;
    if (!e || e.kind !== 'note') return;
    untrack(() => {
      if (e.etag === base) return;
      if (!dirty) {
        draft = e.raw ?? '';
        base = e.etag;
      } else if (!saving) {
        conflict = { theirs: e.raw ?? '', etag: e.etag };
      }
    });
  });

  $effect(() => { try { localStorage.setItem('units', units); } catch {} });

  function onEdit(v: string) {
    draft = v;
    dirty = v !== (entry?.raw ?? '') || base !== entry?.etag;
    clearTimeout(timer);
    if (dirty && !conflict) timer = setTimeout(save, 900);
  }

  function flushSave() {
    if (timer) { clearTimeout(timer); timer = undefined; if (dirty && !conflict) save(); }
  }

  async function save(text = draft, etag = base) {
    clearTimeout(timer);
    timer = undefined;
    if (!entry || saving) return;
    saving = true;
    try {
      const r = await saveFile(path, text, etag);
      base = r.etag;
      dirty = draft !== text;
      conflict = null;
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) {
        await loadVault();
        conflict = { theirs: app.byPath[path]?.raw ?? '', etag: e.body?.etag ?? null };
      } else if (e instanceof ApiError && e.status === 403) {
        toast(e.message, 'error');
        requireWrite();
      } else toast(`Not saved: ${(e as Error).message}`, 'error');
    } finally {
      saving = false;
    }
  }

  async function saveRaw(raw: string) {
    if (!requireWrite()) return;
    draft = raw;
    dirty = true;
    await save(raw, base);
  }

  function keepMine() { save(draft, conflict!.etag); comparing = false; }
  function useTheirs() { draft = conflict!.theirs; base = conflict!.etag; dirty = false; conflict = null; comparing = false; }

  function setMode(m: typeof mode) {
    if (m !== 'read' && !requireWrite()) return;
    if (m === 'read') flushSave();
    mode = m;
  }

  // live preview renders the draft; reading view renders the saved file with scaling applied
  const html = $derived.by(() => {
    if (!entry || entry.kind !== 'note') return '';
    if (mode !== 'read') {
      const fmEnd = draft.match(/^---\r?\n[\s\S]*?\r?\n---[ \t]*(?:\r?\n|$)/);
      return renderMarkdown(fmEnd ? draft.slice(fmEnd[0].length) : draft, path);
    }
    return renderMarkdown(bodyOf(scaledRaw(entry, factor, units), entry.body_line), path);
  });

  const baseYield = $derived(entry ? yieldNumber(entry.frontmatter?.servings) ?? yieldNumber(entry.frontmatter?.yield) : null);
  const yieldUnit = $derived.by(() => {
    const y = String(entry?.frontmatter?.yield ?? entry?.frontmatter?.servings ?? '');
    return y.replace(/^\s*(about\s+|makes\s+)?[\d½¼¾⅓⅔⅛\s/.-]+/i, '').trim() || (entry?.frontmatter?.servings ? 'servings' : '');
  });

  function step(d: number) {
    if (baseYield) {
      const next = Math.max(1, Math.round(baseYield * factor) + d);
      factor = next / baseYield;
    } else {
      const steps = [0.25, 0.5, 1, 1.5, 2, 3, 4];
      const i = steps.findIndex((s) => s >= factor - 1e-9);
      factor = steps[Math.min(steps.length - 1, Math.max(0, (i < 0 ? 2 : i) + d))];
    }
  }

  async function onImages(files: File[]): Promise<string | null> {
    const out: string[] = [];
    for (const f of files) {
      const md = await attachImage(f, entry?.title ?? 'image');
      if (md) out.push(md);
    }
    return out.join('\n') || null;
  }

  // a view-only PDF recipe: the note points at the original PDF
  const origPdf = $derived.by(() => {
    const o = entry?.frontmatter?.original;
    const p = typeof o === 'string' ? vaultTarget(o, '') : null;
    return p && /\.pdf$/i.test(p) && app.byPath[p] ? p : null;
  });

  let converting = $state(false);
  async function convertPdf(pdf: string) {
    if (!requireWrite()) return;
    converting = true;
    try {
      const r = await api('POST', '/api/import/from-vault', { path: pdf });
      enqueueStarted(nameOf(pdf), r.job);
      go('#/import');
    } catch (e: any) { toast(e.message, 'error'); }
    finally { converting = false; }
  }

  // optional AI (only when enabled in Settings on the Mac)
  let aiProposal = $state<string | null>(null);
  let aiBusy = $state(false);
  let asking = $state(false);
  let question = $state('');
  let answer = $state('');
  async function aiTidy() {
    if (!requireWrite() || !entry) return;
    aiBusy = true;
    try { aiProposal = (await api('POST', '/api/ai/tidy', { markdown: entry.raw ?? '' })).markdown; }
    catch (e: any) { toast(e.message, 'error'); }
    finally { aiBusy = false; }
  }
  async function aiAsk(e: Event) {
    e.preventDefault();
    if (!question.trim() || !entry) return;
    aiBusy = true;
    answer = '';
    try { answer = (await api('POST', '/api/ai/ask', { markdown: entry.raw ?? '', question })).answer; }
    catch (err: any) { toast(err.message, 'error'); }
    finally { aiBusy = false; }
  }

  function doPrint() {
    if (app.session.shell) shell('print');
    else window.print();
  }

  function moreMenu(e: MouseEvent) {
    const items = [
      { label: 'Download', run: () => (location.href = downloadUrl(path)) },
      { label: 'Print', run: doPrint },
    ];
    if (canWrite()) items.unshift({ label: 'Rename…', run: () => rename(path) }, { label: 'Move to…', run: () => moveDialog(path) });
    if (app.session.shell) items.push({ label: 'Show in Finder', run: () => shell('reveal', { path }) });
    if (app.session.ai && canWrite() && entry?.kind === 'note') {
      items.push({ label: '✨ Tidy formatting with AI…', run: aiTidy }, { label: '✨ Ask AI about this recipe…', run: () => { asking = true; } });
    }
    if (canWrite()) items.push({ label: 'Move to trash', run: () => remove(path), danger: true } as any);
    openMenu(e, items);
  }

  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'e' && entry?.kind === 'note') {
      e.preventDefault();
      setMode(mode === 'read' ? (wide ? 'split' : 'edit') : 'read');
    }
  }

  window.addEventListener('beforeunload', (e) => { if (dirty) { flushSave(); e.preventDefault(); } });
</script>

<svelte:window onkeydown={onKey} />

{#if !entry}
  <div class="page">
    {#if app.loaded}<p class="muted">“{nameOf(path)}” is not in the recipes folder any more. <a href={folderHash(dirOf(path))}>Back</a></p>
    {:else}<p class="muted">Loading…</p>{/if}
  </div>
{:else}
  <div class="page" class:split={mode === 'split'} class:editing={mode !== 'read'}>
    <header class="toolbar row no-print">
      <nav class="crumbs" aria-label="Breadcrumb">
        <a href={folderHash(dirOf(path))}>{dirOf(path) || 'All recipes'}</a><span class="sep">›</span><span>{nameOf(path)}</span>
      </nav>
      <span class="spacer"></span>
      {#if entry.kind === 'note'}
        <span class="save small muted" aria-live="polite">{saving ? 'Saving…' : dirty ? 'Unsaved' : mode !== 'read' ? 'Saved' : ''}</span>
        <div class="seg" role="group" aria-label="View mode">
          <button class="btn" class:on={mode === 'read'} onclick={() => setMode('read')}>Read</button>
          <button class="btn" class:on={mode === 'edit'} onclick={() => setMode('edit')}>Edit</button>
          {#if wide}<button class="btn" class:on={mode === 'split'} onclick={() => setMode('split')}>Split</button>{/if}
        </div>
      {/if}
      <button class="btn icon" aria-label="More" onclick={moreMenu}><Icon name="more" /></button>
    </header>

    {#if conflict}
      <div class="banner warn row no-print">
        <span>This file was changed somewhere else while you were editing.</span>
        <span class="spacer"></span>
        <button class="btn" onclick={() => (comparing = true)}>Compare</button>
        <button class="btn" onclick={useTheirs}>Use theirs</button>
        <button class="btn primary" onclick={keepMine}>Keep mine</button>
      </div>
    {/if}

    {#if entry.kind === 'note'}
      {#if entry.frontmatter?.needs_review && mode === 'read'}
        <div class="banner row no-print">
          <span>This recipe was converted without clear Ingredients / Directions sections, so scaling and cook mode may be limited. You can tidy it in Edit.</span>
          {#if canWrite()}<button class="btn" onclick={() => saveRaw(setProp(entry.raw ?? '', 'needs_review', undefined))}>Mark as reviewed</button>{/if}
        </div>
      {/if}

      {#if mode === 'read' && entry.recipe}
        <div class="recipebar row no-print">
          <div class="row scaler" role="group" aria-label="Scale recipe">
            <button class="btn icon" onclick={() => step(-1)} aria-label="Fewer"><Icon name="minus" /></button>
            <span class="qty">{#if baseYield}{+(baseYield * factor).toFixed(2)} {yieldUnit}{:else}×{+factor.toFixed(2)}{/if}</span>
            <button class="btn icon" onclick={() => step(1)} aria-label="More servings"><Icon name="plus" /></button>
            {#if factor !== 1}<button class="btn ghost small" onclick={() => (factor = 1)}>Reset</button>{/if}
          </div>
          <select class="field units" bind:value={units} aria-label="Units">
            <option value="original">Units as written</option>
            <option value="us">US units</option>
            <option value="metric">Metric</option>
          </select>
          <span class="spacer"></span>
          <button class="btn primary" onclick={() => (cooking = true)}><Icon name="chef" /> Cook mode</button>
        </div>
      {/if}

      <div class="body">
        {#if mode !== 'read'}
          <div class="editpane no-print">
            <div class="row edtools">
              <button class="btn small" onclick={() => imgInput?.click()}><Icon name="image" size={16} /> Insert image</button>
              <input type="file" accept="image/*" multiple hidden bind:this={imgInput} onchange={async (e) => { const t = e.currentTarget; if (t.files?.length) { const md = await onImages(Array.from(t.files)); if (md) editor?.insert(md + '\n'); } t.value = ''; }} />
              <span class="muted small">Markdown · saves automatically · ⌘E to read</span>
            </div>
            <Editor bind:this={editor} value={draft} onchange={onEdit} onsave={() => save()} onfiles={onImages} />
          </div>
        {/if}
        {#if mode !== 'edit'}
          <article class="reader">
            {#if mode === 'read'}<Properties {entry} editable={canWrite()} onchange={saveRaw} />{/if}
            {#if asking && mode === 'read'}
              <form class="ask card no-print" onsubmit={aiAsk}>
                <div class="row"><input class="field grow" placeholder="Ask about this recipe — e.g. can I make it the day before?" bind:value={question} />
                  <button class="btn primary" disabled={aiBusy || !question.trim()}>{aiBusy ? 'Thinking…' : 'Ask'}</button>
                  <button type="button" class="btn ghost icon" aria-label="Close" onclick={() => { asking = false; answer = ''; }}><Icon name="x" /></button></div>
                {#if answer}<div class="prose answer">{@html renderMarkdown(answer, path)}</div>{/if}
              </form>
            {/if}
            <div class="prose">{@html html}</div>
            {#if mode === 'read'}<Nutrition data={entry.frontmatter?.nutrition} />{/if}
            {#if mode === 'read' && origPdf && !entry.recipe}
              {#if canWrite()}<p class="no-print"><button class="btn" disabled={converting} onclick={() => convertPdf(origPdf!)}>Convert to an editable recipe…</button></p>{/if}
              <PdfView path={origPdf} />
            {/if}
            {#if entry.links?.length && mode === 'read'}<p class="muted small no-print">Links to {entry.links.length} other note{entry.links.length > 1 ? 's' : ''}.</p>{/if}
          </article>
        {/if}
      </div>
    {:else if entry.kind === 'pdf'}
      <h1 class="ftitle">{entry.title}</h1>
      {#if canWrite()}<p class="no-print"><button class="btn" disabled={converting} onclick={() => convertPdf(path)}>Convert to an editable recipe…</button></p>{/if}
      <PdfView {path} />
    {:else if entry.kind === 'image'}
      <h1 class="ftitle">{nameOf(path)}</h1>
      <img class="full" src={fileUrl(path)} alt={nameOf(path)} />
    {:else}
      <h1 class="ftitle">{nameOf(path)}</h1>
      <p class="muted">This file can’t be shown here.</p>
      <a class="btn" href={downloadUrl(path)}>Download</a>
    {/if}
  </div>

  {#if aiProposal !== null}
    <Merge theirs={entry.raw ?? ''} mine={aiProposal} leftLabel="Current recipe" rightLabel="AI suggestion (edit freely)" accept="Save suggestion"
      onaccept={(t) => { aiProposal = null; saveRaw(t); }} oncancel={() => (aiProposal = null)} />
  {/if}
  {#if aiBusy && !asking}<div class="busy" role="status">Asking AI…</div>{/if}
  {#if comparing && conflict}
    <Merge theirs={conflict.theirs} mine={draft} leftLabel="Version saved elsewhere" onaccept={(t) => { draft = t; keepMine(); }} oncancel={() => (comparing = false)} />
  {/if}
  {#if cooking && entry.recipe}
    <CookMode {entry} {factor} {units} onclose={() => (cooking = false)} />
  {/if}
{/if}

<style>
  .page { padding: 16px 28px 64px; }
  .toolbar { position: sticky; top: 0; z-index: 5; background: var(--bg); padding: 6px 0 10px; }
  .crumbs { display: flex; gap: 6px; min-width: 0; color: var(--muted); }
  .crumbs a { color: var(--muted); text-decoration: none; }
  .crumbs > span:last-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text); }
  .seg { display: flex; }
  .seg .btn { border-radius: 0; }
  .seg .btn:first-child { border-radius: 8px 0 0 8px; }
  .seg .btn:last-child { border-radius: 0 8px 8px 0; }
  .seg .btn + .btn { border-left: 0; }
  .banner { background: var(--p-yellow); border-radius: var(--radius); padding: 10px 14px; margin: 4px 0 14px; font-size: 14px; max-width: 760px; }
  .banner.warn { background: var(--p-red); max-width: none; }
  .recipebar { margin: 0 0 16px; max-width: 760px; padding: 8px 10px; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); }
  .scaler .qty { min-width: 90px; text-align: center; font-weight: 600; }
  .units { width: auto; }
  .body { display: block; }
  .split .body { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; align-items: start; }
  .editpane { display: flex; flex-direction: column; gap: 8px; height: calc(100vh - 110px); position: sticky; top: 60px; }
  .editing:not(.split) .editpane { height: calc(100vh - 110px); }
  .edtools { gap: 10px; }
  .split .reader { max-height: calc(100vh - 110px); overflow-y: auto; padding-right: 8px; }
  .ftitle { margin: 6px 0 14px; }
  .ask { padding: 10px 12px; margin-bottom: 16px; max-width: 760px; display: grid; gap: 8px; }
  .ask .grow { flex: 1; min-width: 200px; }
  .answer { font-size: 15px; }
  .busy { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.25); display: grid; place-items: center; color: #fff; font-size: 18px; z-index: 70; }
  .full { max-width: 100%; border-radius: var(--radius); }
  @media (max-width: 899px) {
    .page { padding: 6px 14px 96px; }
    .crumbs { display: none; }
    .editpane { height: calc(100vh - 170px); top: 52px; }
    .recipebar .spacer { display: none; }
    .recipebar .btn.primary { width: 100%; justify-content: center; }
  }
</style>
