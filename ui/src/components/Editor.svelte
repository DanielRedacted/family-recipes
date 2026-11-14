<script lang="ts">
  import { onMount } from 'svelte';
  import { EditorView, keymap } from '@codemirror/view';
  import { basicSetup } from 'codemirror';
  import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
  import { yamlFrontmatter } from '@codemirror/lang-yaml';
  import { Compartment, EditorState, type TransactionSpec } from '@codemirror/state';
  import { indentWithTab } from '@codemirror/commands';
  import Icon from './Icon.svelte';
  import { wikiLinks } from '../lib/complete';
  import { ingredientSuggest, loadIngredients } from '../lib/ingredients.svelte';
  import { livePreview } from '../lib/livePreview';
  import { toggleLines, toggleWrap, type LineKind } from '../lib/mdCommands';
  import { vaultTarget } from '../lib/markdown';
  import { fileUrl } from '../lib/api';
  import { app, go, noteHash, resolveLink, toast } from '../lib/state.svelte';
  import { choose } from '../lib/dialog.svelte';

  let { value, onchange, onsave, onfiles, readonly = false, notePath = '', toolbar = true }: {
    value: string;
    onchange: (v: string) => void;
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
  let formatted = $state((() => { try { return localStorage.getItem('editorMode') !== 'source'; } catch { return true; } })());

  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: '15px', backgroundColor: 'var(--surface)', color: 'var(--text)' },
    '.cm-content': { fontFamily: 'var(--mono)', padding: '16px 0', caretColor: 'var(--accent)' },
    '.cm-line': { padding: '0 18px' },
    '.cm-gutters': { backgroundColor: 'var(--surface)', borderRight: '1px solid var(--border)', color: 'var(--muted)' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--sunken)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--accent-soft) !important' },
    '.cm-cursor': { borderLeftColor: 'var(--accent)' },
    '&.cm-formatted .cm-gutters': { display: 'none' },
    '&.cm-formatted .cm-activeLine': { backgroundColor: 'transparent' },
    '&.cm-formatted .cm-line': { padding: '0 22px' },
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
  });
  const lookExt = (on: boolean) => (on ? [preview, EditorView.editorAttributes.of({ class: 'cm-formatted' })] : []);

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
            { key: 'Mod-b', run: () => apply(toggleWrap(view.state, '**')) },
            { key: 'Mod-i', run: () => apply(toggleWrap(view.state, '*')) },
          ]),
          EditorView.updateListener.of((u) => { if (u.docChanged) onchange(u.state.doc.toString()); }),
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

  function apply(spec: TransactionSpec) {
    view.dispatch(spec, { scrollIntoView: true });
    view.focus();
    return true;
  }
  const lines = (k: LineKind) => apply(toggleLines(view.state, k));

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
    <div class="bar" role="toolbar" aria-label="Formatting">
      <div class="seg" role="group" aria-label="Editor style">
        <button type="button" class="btn small" class:on={formatted} onclick={() => (formatted = true)} title="Formatted: Markdown shown only where you type">Formatted</button>
        <button type="button" class="btn small" class:on={!formatted} onclick={() => (formatted = false)} title="Plain Markdown text">Markdown</button>
      </div>
      <span class="gap"></span>
      <button type="button" class="tb" title="Section heading" aria-label="Section heading" onclick={() => lines('h2')}><Icon name="heading" size={17} /></button>
      <button type="button" class="tb" title="Bold (⌘B)" aria-label="Bold" onclick={() => apply(toggleWrap(view.state, '**'))}><Icon name="bold" size={17} /></button>
      <button type="button" class="tb" title="Italic (⌘I)" aria-label="Italic" onclick={() => apply(toggleWrap(view.state, '*'))}><Icon name="italic" size={17} /></button>
      <button type="button" class="tb" title="Bulleted list" aria-label="Bulleted list" onclick={() => lines('bullet')}><Icon name="list" size={17} /></button>
      <button type="button" class="tb" title="Numbered list" aria-label="Numbered list" onclick={() => lines('number')}><Icon name="list-ordered" size={17} /></button>
      <button type="button" class="tb" title="Checklist" aria-label="Checklist" onclick={() => lines('task')}><Icon name="check-square" size={17} /></button>
      <button type="button" class="tb" title="Quote" aria-label="Quote" onclick={() => lines('quote')}><Icon name="quote" size={17} /></button>
      <button type="button" class="tb" title="Link to another recipe ([[)" aria-label="Link to another recipe" onclick={linkRecipe}><Icon name="link" size={17} /></button>
      {#if onfiles}
        <button type="button" class="tb" title="Insert picture" aria-label="Insert picture" onclick={() => imgInput?.click()}><Icon name="image" size={17} /></button>
        <input type="file" accept="image/*" multiple hidden bind:this={imgInput} onchange={(e) => { pickedImages(e.currentTarget.files); e.currentTarget.value = ''; }} />
      {/if}
    </div>
  {/if}
  <div class="editor" bind:this={host}></div>
</div>

<style>
  .wrap { height: 100%; min-height: 300px; display: flex; flex-direction: column; border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; background: var(--surface); }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 2px; padding: 5px 8px; border-bottom: 1px solid var(--border); background: var(--bg); }
  .gap { width: 8px; }
  .seg { display: flex; }
  .seg .btn { border-radius: 0; }
  .seg .btn:first-child { border-radius: 7px 0 0 7px; }
  .seg .btn:last-child { border-radius: 0 7px 7px 0; border-left: 0; }
  .tb { border: 0; background: none; color: var(--text); width: 32px; height: 30px; border-radius: 6px; display: grid; place-items: center; cursor: pointer; }
  .tb:hover { background: var(--sunken); }
  .editor { flex: 1; min-height: 0; }
  .editor :global(.cm-editor) { height: 100%; }
  .editor :global(.cm-scroller) { overflow: auto; }
  @media (max-width: 899px) { .tb { width: 36px; height: 36px; } }
</style>
