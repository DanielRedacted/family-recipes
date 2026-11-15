<script lang="ts">
  import Icon from './Icon.svelte';
  import { onMount, untrack } from 'svelte';
  import { app, canWrite, go, folderHash, noteHash, dirOf, nameOf, toast, loadVault, setLeaveGuard, clearLeaveGuard, resolveLink } from '../lib/state.svelte';
  import { fileUrl, saveFile, ApiError, api } from '../lib/api';
  import Crumbs from './Crumbs.svelte';
  import { renderMarkdown, scaledRaw, bodyOf, vaultTarget } from '../lib/markdown';
  import { formatNum, parseYield, portionMultipliers, resolvePortion, yieldLabel, type Units } from '../lib/scale';
  import { openMenu, type MenuItem } from '../lib/menu.svelte';
  import { entryItems, requireWrite, attachImage, shell } from '../lib/ops';
  import { confirm3 } from '../lib/dialog.svelte';
  import Editor from './Editor.svelte';
  import Merge from './Merge.svelte';
  import Properties, { setProp } from './Properties.svelte';
  import CookMode from './CookMode.svelte';
  import PdfView from './PdfView.svelte';
  import Nutrition from './Nutrition.svelte';
  import { enqueueStarted } from '../lib/importer.svelte';

  let { path, mode: routeMode }: { path: string; mode?: string } = $props();

  const entry = $derived(app.byPath[path]);
  const mq = typeof matchMedia !== 'undefined' ? matchMedia('(min-width: 1100px)') : null;
  let wide = $state(mq?.matches ?? false);
  let mode = $state<'read' | 'edit' | 'split'>('read');
  let draft = $state('');
  let base = $state<string | null>(null); // etag of the text the draft started from
  let dirty = $state(false);
  /** the draft after the editor only re-spaced tables: not a change worth asking about */
  let tidied: string | null = null;
  let saving = $state(false);
  let conflict = $state<{ theirs: string; etag: string | null } | null>(null);
  let comparing = $state(false);
  let cooking = $state(false);
  let factor = $state(1);
  /** the chosen column of an ingredient table with alternative portions (Single / Double…) */
  let portion = $state(0);
  // "original" (units as written) is gone: recipes are written in one spelling, shown as US by default
  let units = $state<Units>((() => { try { return localStorage.getItem('units') === 'metric' ? 'metric' : 'us'; } catch { return 'us'; } })());
  let editor: Editor | undefined = $state();

  // start state (the component is re-created for each note)
  $effect(() => {
    path;
    untrack(() => {
      mode = routeMode === 'edit' && canWrite() ? 'edit' : 'read';
      const e = app.byPath[path];
      draft = e?.raw ?? '';
      base = e?.etag ?? null;
      dirty = false;
      tidied = null;
      conflict = null;
      factor = 1;
      portion = 0;
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

  // Nothing is written until Save (or ⌘S). Leaving with unsaved changes asks first.
  function onEdit(v: string, tidy = false) {
    draft = v;
    if (tidy && !dirty) tidied = v;
    dirty = (v !== (entry?.raw ?? '') && v !== tidied) || base !== entry?.etag;
  }

  async function save(text = draft, etag = base) {
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

  /** true when it is fine to leave the editor (saved, discarded, or nothing to save). */
  async function confirmLeave(): Promise<boolean> {
    if (!dirty) return true;
    const r = await confirm3('Save your changes?', `“${entry?.title ?? nameOf(path)}” has changes that are not saved yet.`, 'Save', 'Don’t save');
    if (r === 'ok') { await save(); return !dirty && !conflict; }
    if (r === 'alt') { draft = entry?.raw ?? ''; base = entry?.etag ?? null; dirty = false; conflict = null; return true; }
    return false;
  }

  onMount(() => {
    setLeaveGuard(confirmLeave);
    const unload = (e: BeforeUnloadEvent) => { if (dirty) e.preventDefault(); };
    const media = () => { wide = mq?.matches ?? false; if (!wide && mode === 'split') mode = 'edit'; };
    window.addEventListener('beforeunload', unload);
    mq?.addEventListener('change', media);
    return () => { clearLeaveGuard(confirmLeave); window.removeEventListener('beforeunload', unload); mq?.removeEventListener('change', media); };
  });

  /** Properties panel, "Mark as reviewed", AI tidy: one immediate, deliberate save. */
  async function saveRaw(raw: string) {
    if (!requireWrite()) return;
    draft = raw;
    dirty = true;
    await save(raw, base);
  }

  function keepMine() { save(draft, conflict!.etag); comparing = false; }
  function useTheirs() { draft = conflict!.theirs; base = conflict!.etag; dirty = false; conflict = null; comparing = false; }

  async function setMode(m: typeof mode) {
    if (m !== 'read' && !requireWrite()) return;
    if (m === 'read' && !(await confirmLeave())) return;
    mode = m;
  }
  const startEditing = () => setMode('edit');

  // live preview renders the draft; reading view renders the saved file with scaling applied
  const html = $derived.by(() => {
    if (!entry || entry.kind !== 'note') return '';
    if (mode !== 'read') {
      const fmEnd = draft.match(/^---\r?\n[\s\S]*?\r?\n---[ \t]*(?:\r?\n|$)/);
      return renderMarkdown(fmEnd ? draft.slice(fmEnd[0].length) : draft, path);
    }
    return renderMarkdown(bodyOf(scaledRaw(entry, factor, units, portion), entry.body_line), path);
  });

  const baseYield = $derived(entry ? parseYield(entry.frontmatter?.servings) ?? parseYield(entry.frontmatter?.yield) : null);
  const yieldFallbackUnit = $derived(parseYield(entry?.frontmatter?.servings) ? 'servings' : '');

  // a table with alternative portions: a chip per portion, with what it makes compared with the first
  const portionGroup = $derived(entry?.recipe?.ingredients.find((g) => g.portions?.length));
  const mults = $derived(portionGroup ? portionMultipliers(portionGroup.items, portionGroup.portions!) : []);
  const chips = $derived((portionGroup?.portions ?? []).map((label, k) => ({
    label: label || (mults[k] !== null ? '' : `Version ${k + 1}`),
    mult: mults[k] !== null ? '×' + formatNum(mults[k]!) : '',
  })));
  // the chip shown as chosen: a size whose multiplier the scaler reached, else the one picked
  const shown = $derived(mults.length ? resolvePortion(mults, portion, factor).col : 0);
  function pick(k: number) {
    portion = k;
    if (mults[k] !== null) factor = mults[k]!;
  }

  function step(d: number) {
    if (baseYield) {
      const by = baseYield.dozen ? 6 : 1;
      const next = Math.max(by, Math.round((baseYield.n * factor) / by) * by + d * by);
      factor = next / baseYield.n;
    } else {
      const steps = [...new Set([0.25, 0.5, 1, 1.5, 2, 3, 4, ...mults.filter((m): m is number => m !== null)])].sort((a, b) => a - b);
      const i = steps.findIndex((s) => s >= factor - 1e-9);
      factor = steps[Math.min(steps.length - 1, Math.max(0, (i < 0 ? steps.indexOf(1) : i) + d))];
    }
    if (mults.length) portion = resolvePortion(mults, portion, factor).col;
  }

  async function onImages(files: File[]): Promise<string | null> {
    const out: string[] = [];
    for (const f of files) {
      const md = await attachImage(f, entry?.title ?? 'image');
      if (md) out.push(md);
    }
    return out.join('\n') || null;
  }

  // notes whose [[links]] point here
  const mentionedIn = $derived(entry ? app.files.filter((f) => f.path !== path && f.links?.some((l) => resolveLink(l)?.path === path)) : []);

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
    const isNote = entry?.kind === 'note';
    const items: MenuItem[] = [];
    if (isNote && canWrite()) {
      if (mode === 'read') items.push({ label: 'Edit', run: startEditing });
      else {
        items.push({ label: 'Done editing', run: () => setMode('read') });
        if (wide) items.push({ label: mode === 'split' ? 'Hide preview' : 'Show preview alongside', run: () => setMode(mode === 'split' ? 'edit' : 'split') });
      }
    }
    const rest = entryItems(path, false);
    if (rest.length) rest[0] = { ...rest[0], sep: items.length > 0 };
    items.push(...rest.filter((i) => i.label !== 'Move to trash'));
    items.push({ label: 'Print', run: doPrint });
    if (app.session.ai && canWrite() && isNote) {
      items.push({ label: 'Tidy formatting with AI…', run: aiTidy, sep: true }, { label: 'Ask AI about this recipe…', run: () => { asking = true; } });
    }
    const trash = rest.find((i) => i.label === 'Move to trash');
    if (trash) items.push(trash);
    openMenu(e, items);
  }

  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'e' && entry?.kind === 'note') {
      e.preventDefault();
      if (mode === 'read') startEditing(); else setMode('read');
    }
  }
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
      <Crumbs dir={dirOf(path)} leaf={entry.kind === 'note' ? entry.title : nameOf(path)} />
      <span class="spacer"></span>
      {#if entry.kind === 'note' && mode !== 'read'}
        <span class="save small muted" aria-live="polite">{saving ? 'Saving…' : dirty ? 'Unsaved changes' : 'Saved'}</span>
        <button class="btn" onclick={() => setMode('read')}>Done</button>
        <button class="btn primary" disabled={!dirty || saving || !!conflict} onclick={() => save()} title="Save (⌘S)"><Icon name="check" size={16} /> Save</button>
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
            <span class="qty">{#if baseYield}{yieldLabel(baseYield, factor, yieldFallbackUnit)}{:else}×{+factor.toFixed(2)}{/if}</span>
            <button class="btn icon" onclick={() => step(1)} aria-label="More servings"><Icon name="plus" /></button>
            {#if factor !== 1}<button class="btn ghost small" onclick={() => { factor = 1; portion = 0; }}>Reset</button>{/if}
          </div>
          {#if chips.length}
            <div class="row portions" role="radiogroup" aria-label="Portion">
              {#each chips as c, k}
                <button class="chip" role="radio" aria-checked={shown === k} class:on={shown === k} onclick={() => pick(k)}>
                  {c.label}{#if c.mult}{c.label ? ' ' : ''}<span class="mult">{c.mult}</span>{/if}
                </button>
              {/each}
            </div>
          {/if}
          <select class="field units" bind:value={units} aria-label="Units">
            <option value="us">US</option>
            <option value="metric">Metric</option>
          </select>
          <span class="spacer"></span>
          <button class="btn primary" onclick={() => (cooking = true)}><Icon name="chef" /> Cook mode</button>
        </div>
      {/if}

      <div class="body">
        {#if mode !== 'read'}
          <div class="editpane no-print">
            <Editor bind:this={editor} value={draft} onchange={onEdit} onsave={() => save()} onfiles={onImages} notePath={path} />
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
            {#if mentionedIn.length && mode === 'read'}
              <aside class="mentions no-print">
                <h2>Mentioned in</h2>
                <ul>{#each mentionedIn as m (m.path)}<li><a href={noteHash(m.path)}>{m.title}</a> <span class="muted small">{dirOf(m.path)}</span></li>{/each}</ul>
              </aside>
            {/if}
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
      {#if app.session.shell}<button class="btn" onclick={() => shell('reveal', { path })}>Show in Finder</button>{/if}
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
    <CookMode {entry} {factor} {units} {portion} onclose={() => (cooking = false)} />
  {/if}
{/if}

<style>
  .page { padding: 16px 24px 64px; }
  .toolbar { position: sticky; top: 0; z-index: 5; background: var(--bg); padding: 8px 0 12px; }
  .mentions { margin: 32px 0 0; padding-top: 12px; border-top: 1px solid var(--border); max-width: 760px; }
  .mentions h2 { font: 600 14px var(--sans); color: var(--muted); margin: 0 0 8px; }
  .mentions ul { margin: 0; padding-left: 20px; }
  .banner { background: var(--p-yellow); border-radius: var(--radius); padding: 12px 16px; margin: 4px 0 16px; font-size: 14px; max-width: 760px; }
  .banner.warn { background: var(--p-red); max-width: none; }
  .recipebar { margin: 0 0 16px; max-width: 760px; padding: 8px 12px; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); }
  .scaler .qty { min-width: 90px; text-align: center; font-weight: 600; }
  .portions { gap: 4px; }
  .chip { border: 1px solid var(--border); background: var(--surface); border-radius: 16px; padding: 4px 12px; min-height: 32px; cursor: pointer; font-size: 14px; }
  .chip.on { background: var(--accent-soft); border-color: var(--accent); color: var(--accent-ink); font-weight: 600; }
  .chip .mult { margin-left: 2px; color: var(--muted); font-weight: 500; }
  .units { width: auto; }
  .body { display: block; }
  .split .body { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; align-items: start; }
  .editpane { display: flex; flex-direction: column; gap: 8px; height: calc(100vh - 110px); position: sticky; top: 60px; }
  .editing:not(.split) .editpane { height: calc(100vh - 110px); max-width: 880px; }
  .split .reader { max-height: calc(100vh - 110px); overflow-y: auto; padding-right: 8px; }
  .ftitle { margin: 8px 0 16px; }
  .ask { padding: 12px; margin-bottom: 16px; max-width: 760px; display: grid; gap: 8px; }
  .ask .grow { flex: 1; min-width: 200px; }
  .answer { font-size: 15px; }
  .busy { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.25); display: grid; place-items: center; color: #fff; font-size: 18px; z-index: 70; }
  .full { max-width: 100%; border-radius: var(--radius); }
  @media (max-width: 899px) {
    .page { padding: 8px 16px 96px; }
    .editpane { height: calc(100vh - 170px); top: 52px; }
    .recipebar .spacer { display: none; }
    .recipebar .btn.primary { width: 100%; justify-content: center; }
  }
</style>
