<script lang="ts">
  import { onMount } from 'svelte';
  import { EditorView, keymap } from '@codemirror/view';
  import { basicSetup } from 'codemirror';
  import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
  import { tags } from '@lezer/highlight';
  import { yamlFrontmatter } from '@codemirror/lang-yaml';
  import { Compartment, EditorState, Transaction, type TransactionSpec } from '@codemirror/state';
  import { historyKeymap, indentWithTab } from '@codemirror/commands';
  import { insertEmptyMarkdownTable, markdownTables, TableStyle, TableTheme } from 'codemirror-markdown-tables';
  import Icon from './Icon.svelte';
  import { wikiLinks } from '../lib/complete';
  import { ingredientSuggest, loadIngredients } from '../lib/ingredients.svelte';
  import { livePreview } from '../lib/livePreview';
  import { lineKind, setLines, toggleLines, toggleWrap, type LineKind } from '../lib/mdCommands';
  import { vaultTarget } from '../lib/markdown';
  import { fileUrl } from '../lib/api';
  import { app, go, noteHash, resolveLink, toast } from '../lib/state.svelte';
  import { choose } from '../lib/dialog.svelte';
  import { openExternal } from '../lib/ops';

  let { value, onchange, onsave, onfiles, readonly = false, notePath = '', toolbar = true }: {
    value: string;
    /** `tidy`: only table spacing changed (not a user edit) */
    onchange: (v: string, tidy?: boolean) => void;
    onsave?: () => void;
    onfiles?: (files: File[]) => Promise<string | null>;
    readonly?: boolean;
    /** the note being edited, for resolving relative image paths */
    notePath?: string;
    toolbar?: boolean;
  } = $props();

  let host: HTMLDivElement;
  let view: EditorView;
  let imgInput: HTMLInputElement | undefined = $state();
  const look = new Compartment();
  let textType = $state<'p' | 'h1' | 'h2' | 'h3'>('p');
  /** a table cell's editor has focus (toolbar commands go to it) */
  let inCell = $state(false);
  let formatted = $state((() => { try { return localStorage.getItem('editorMode') !== 'source'; } catch { return true; } })());

  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: '15px', backgroundColor: 'var(--surface)', color: 'var(--text)' },
    '.cm-content': { fontFamily: 'var(--mono)', padding: '16px 0', caretColor: 'var(--accent)' },
    '.cm-line': { padding: '0 16px' },
    '.cm-gutters': { backgroundColor: 'var(--surface)', borderRight: '1px solid var(--border)', color: 'var(--muted)' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--sunken)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--accent-soft) !important' },
    '.cm-cursor': { borderLeftColor: 'var(--accent)' },
    '&.cm-formatted .cm-gutters': { display: 'none' },
    '&.cm-formatted .cm-activeLine': { backgroundColor: 'transparent' },
    '&.cm-formatted .cm-line': { padding: '0 24px' },
    // table cells: header text is not a Markdown heading
    '.tbl-header-cell span': { textDecoration: 'none' },
    '.tbl-cell-view .cm-md-mark': { display: 'none' },
    '&.cm-formatted .tbl-table-widget': { paddingLeft: '22px', paddingRight: '22px' },
  });

  const noteDir = () => (notePath.includes('/') ? notePath.slice(0, notePath.lastIndexOf('/')) : '');
  const preview = livePreview({
    imageUrl: (src) => {
      if (/^https?:\/\//i.test(src)) return src;
      const p = vaultTarget(src, noteDir());
      return p ? fileUrl(p) : null;
    },
    openLink: (t) => {
      const e = resolveLink(t);
      if (e) go(noteHash(e.path));
      else toast(`No recipe called “${t}”`, 'error');
    },
    openHref: (href) => {
      if (/^https?:\/\//i.test(href)) return openExternal(href);
      const p = vaultTarget(href, noteDir());
      if (p !== null) go(noteHash(p));
    },
  });
  // tables are HTML tables with a small editor in each cell (codemirror-markdown-tables): clicks,
  // selection and Tab/Enter/arrows work like any table; marks stay hidden inside cells too
  const wrapKeys = keymap.of([
    { key: 'Mod-b', run: (v) => apply(toggleWrap(v.state, '**'), v) },
    { key: 'Mod-i', run: (v) => apply(toggleWrap(v.state, '*'), v) },
  ]);
  const tables = markdownTables({
    extensions: [preview, wrapKeys],
    globalKeyBindings: historyKeymap,
    markdownConfig: {},
    theme: TableTheme.light.with({
      '--tbl-theme-border-color': 'var(--border)', '--tbl-theme-border-hover-color': 'var(--accent-soft)',
      '--tbl-theme-border-active-color': 'var(--accent)', '--tbl-theme-outline-color': 'var(--accent)',
      '--tbl-theme-header-row-background': 'var(--sunken)', '--tbl-theme-row-background': 'var(--surface)',
      '--tbl-theme-odd-row-background': 'var(--surface)', '--tbl-theme-even-row-background': 'var(--surface)',
      '--tbl-theme-text-color': 'var(--text)', '--tbl-theme-menu-background': 'var(--surface)',
      '--tbl-theme-menu-border-color': 'var(--border)', '--tbl-theme-menu-hover-background': 'var(--sunken)',
      '--tbl-theme-menu-text-color': 'var(--text)', '--tbl-theme-menu-hover-text-color': 'var(--text)',
      '--tbl-theme-select-all-focus-overlay': 'color-mix(in srgb, var(--accent) 18%, transparent)',
      '--tbl-theme-select-all-blur-overlay': 'color-mix(in srgb, var(--accent) 8%, transparent)',
    }),
    style: TableStyle.default.with({ '--tbl-style-font-family': 'var(--serif)', '--tbl-style-font-size': '16px', '--tbl-style-menu-font-family': 'var(--sans)' }),
  });
  // the package lays every table out in one padded style ("table.format") before drawing it; that
  // is whitespace only, so it is passed on as a tidy-up and doesn't count as an edit
  // matches the package's private annotation value; recheck when upgrading it
  const tidyOnly = (tr: Transaction) =>
    ((tr as unknown as { annotations?: { value: unknown }[] }).annotations ?? []).some((a) => a.value === 'table.format');
  // …and it stays out of the undo history, so ⌘Z only undoes what was typed
  const tidyOutOfHistory = EditorState.transactionExtender.of((tr) => (tidyOnly(tr) ? { annotations: Transaction.addToHistory.of(false) } : null));
  // a cell that isn't being edited is drawn as highlighted text by the package: Markdown marks
  // (** _ ` [ ]( #) carry the processingInstruction tag there, and are hidden like everywhere else
  const cellMarks = syntaxHighlighting(HighlightStyle.define([
    { tag: tags.processingInstruction, class: 'cm-md-mark' },
    { tag: tags.strong, fontWeight: 'bold' },
    { tag: tags.emphasis, fontStyle: 'italic' },
    { tag: tags.strikethrough, textDecoration: 'line-through' },
  ]));
  const lookExt = (on: boolean) => (on ? [preview, tables, tidyOutOfHistory, cellMarks, EditorView.editorAttributes.of({ class: 'cm-formatted' })] : []);

  async function dropFiles(files: File[], pos: number) {
    if (!onfiles || !files.length) return false;
    const text = await onfiles(files);
    if (text) view.dispatch({ changes: { from: pos, insert: text + '\n' } });
    return true;
  }

  onMount(() => {
    loadIngredients();
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          basicSetup,
          yamlFrontmatter({ content: markdown({ base: markdownLanguage }) }),
          EditorView.lineWrapping,
          theme,
          look.of(lookExt(formatted)),
          EditorState.readOnly.of(readonly),
          EditorState.languageData.of(() => [{ autocomplete: wikiLinks }, { autocomplete: ingredientSuggest }]),
          keymap.of([
            indentWithTab,
            { key: 'Mod-s', preventDefault: true, run: () => { onsave?.(); return true; } },
            { key: 'Mod-b', run: (v) => apply(toggleWrap(v.state, '**'), v) },
            { key: 'Mod-i', run: (v) => apply(toggleWrap(v.state, '*'), v) },
          ]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange(u.state.doc.toString(), u.transactions.every(tidyOnly));
            if (u.docChanged || u.selectionSet) textType = lineKind(u.state);
          }),
          EditorView.domEventHandlers({
            drop: (e, v) => {
              const files = Array.from(e.dataTransfer?.files ?? []).filter((f) => f.type.startsWith('image/'));
              if (!files.length) return false;
              e.preventDefault();
              const pos = v.posAtCoords({ x: e.clientX, y: e.clientY }) ?? v.state.selection.main.head;
              dropFiles(files, pos);
              return true;
            },
            paste: (e, v) => {
              const files = Array.from(e.clipboardData?.files ?? []).filter((f) => f.type.startsWith('image/'));
              if (!files.length) return false;
              e.preventDefault();
              dropFiles(files, v.state.selection.main.head);
              return true;
            },
          }),
        ],
      }),
    });
    return () => view.destroy();
  });

  // external replacement of the document (e.g. reload after a change on disk)
  $effect(() => {
    const v = value;
    if (view && v !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: v } });
    }
  });

  $effect(() => {
    const on = formatted;
    try { localStorage.setItem('editorMode', on ? 'formatted' : 'source'); } catch {}
    view?.dispatch({ effects: look.reconfigure(lookExt(on)) });
  });

  /** the editor toolbar commands act on: a focused table cell's editor, else the note's */
  function target(): EditorView {
    const el = document.activeElement?.closest('.cm-editor');
    const v = el && host.contains(el) ? EditorView.findFromDOM(el as HTMLElement) : null;
    return v ?? view;
  }
  function apply(spec: TransactionSpec, v = target()) {
    v.dispatch(spec, { scrollIntoView: true });
    v.focus();
    return true;
  }
  const wrap = (m: string) => { const v = target(); apply(toggleWrap(v.state, m), v); };
  const lines = (k: LineKind) => apply(toggleLines(view.state, k), view);
  const addTable = () => { insertEmptyMarkdownTable()({ state: view.state, dispatch: (tr) => view.dispatch(tr) }); view.focus(); };
  const trackCell = () => { const v = target(); inCell = v !== view; };

  export function insert(text: string) {
    const pos = view.state.selection.main.head;
    view.dispatch({ changes: { from: pos, insert: text }, selection: { anchor: pos + text.length } });
    view.focus();
  }

  async function linkRecipe() {
    const titles = [...new Set(app.files.filter((f) => f.kind === 'note' && f.path !== notePath).map((f) => f.title))].sort((a, b) => a.localeCompare(b));
    const t = await choose('Link to another recipe', titles, titles[0] ?? '', 'Insert link');
    if (t) insert(`[[${t}]]`);
  }

  async function pickedImages(files: FileList | null) {
    if (!files?.length || !onfiles) return;
    const md = await onfiles(Array.from(files));
    if (md) insert(md + '\n');
  }
</script>

<div class="wrap">
  {#if toolbar && !readonly}
    <!-- mousedown keeps focus where it was, so B / I apply to the table cell being edited -->
    <div class="bar" role="toolbar" aria-label="Formatting" tabindex="-1" onmousedown={(e) => { if (!(e.target as HTMLElement).closest('select')) e.preventDefault(); }}>
      <div class="seg" role="group" aria-label="Editor style">
        <button type="button" class="btn small" class:on={formatted} onclick={() => (formatted = true)} title="Formatted: Markdown shown only where you type">Formatted</button>
        <button type="button" class="btn small" class:on={!formatted} onclick={() => (formatted = false)} title="Plain Markdown text">Markdown</button>
      </div>
      <span class="gap"></span>
      <select class="field small ttype" aria-label="Text type" value={textType}
        disabled={inCell} onchange={(e) => { apply(setLines(view.state, e.currentTarget.value as 'p' | 'h1' | 'h2' | 'h3'), view); }}>
        <option value="p">Body</option>
        <option value="h1">Title</option>
        <option value="h2">Heading</option>
        <option value="h3">Subheading</option>
      </select>
      <button type="button" class="tb" title="Bold (⌘B)" aria-label="Bold" onclick={() => wrap('**')}><Icon name="bold" size={17} /></button>
      <button type="button" class="tb" title="Italic (⌘I)" aria-label="Italic" onclick={() => wrap('*')}><Icon name="italic" size={17} /></button>
      <button type="button" class="tb" title="Bulleted list" aria-label="Bulleted list" onclick={() => lines('bullet')}><Icon name="list" size={17} /></button>
      <button type="button" class="tb" title="Numbered list" aria-label="Numbered list" onclick={() => lines('number')}><Icon name="list-ordered" size={17} /></button>
      <button type="button" class="tb" title="Checklist" aria-label="Checklist" onclick={() => lines('task')}><Icon name="check-square" size={17} /></button>
      <button type="button" class="tb" title="Quote" aria-label="Quote" onclick={() => lines('quote')}><Icon name="quote" size={17} /></button>
      <button type="button" class="tb" title="Insert table" aria-label="Insert table" disabled={inCell} onclick={addTable}><Icon name="table" size={17} /></button>
      <button type="button" class="tb" title="Link to another recipe ([[)" aria-label="Link to another recipe" onclick={linkRecipe}><Icon name="link" size={17} /></button>
      {#if onfiles}
        <button type="button" class="tb" title="Insert picture" aria-label="Insert picture" onclick={() => imgInput?.click()}><Icon name="image" size={17} /></button>
        <input type="file" accept="image/*" multiple hidden bind:this={imgInput} onchange={(e) => { pickedImages(e.currentTarget.files); e.currentTarget.value = ''; }} />
      {/if}
    </div>
  {/if}
  <div class="editor" bind:this={host} onfocusin={trackCell} onfocusout={() => setTimeout(trackCell)}></div>
</div>

<style>
  .wrap { height: 100%; min-height: 300px; display: flex; flex-direction: column; border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; background: var(--surface); }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 2px; padding: 4px 8px; border-bottom: 1px solid var(--border); background: var(--bg); }
  .gap { width: 8px; }
  .seg { display: flex; }
  .seg .btn { border-radius: 0; }
  .seg .btn:first-child { border-radius: 7px 0 0 7px; }
  .seg .btn:last-child { border-radius: 0 7px 7px 0; border-left: 0; }
  .tb { border: 0; background: none; color: var(--text); width: 32px; height: 32px; border-radius: 6px; display: grid; place-items: center; cursor: pointer; }
  .tb:hover { background: var(--sunken); }
  .ttype { width: auto; min-height: 32px; padding: 2px 8px; font-size: 13px; margin-right: 4px; }
  .tb:disabled { opacity: 0.4; cursor: default; }
  .editor { flex: 1; min-height: 0; }
  .editor :global(.cm-editor) { height: 100%; }
  .editor :global(.cm-scroller) { overflow: auto; }
  @media (max-width: 899px) { .tb { width: 36px; height: 36px; } }
</style>
