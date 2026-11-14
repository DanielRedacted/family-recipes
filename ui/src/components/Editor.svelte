<script lang="ts">
  import { onMount } from 'svelte';
  import { EditorView, keymap } from '@codemirror/view';
  import { basicSetup } from 'codemirror';
  import { markdown } from '@codemirror/lang-markdown';
  import { EditorState } from '@codemirror/state';
  import { indentWithTab } from '@codemirror/commands';

  let { value, onchange, onsave, onfiles, readonly = false }: {
    value: string;
    onchange: (v: string) => void;
    onsave?: () => void;
    onfiles?: (files: File[]) => Promise<string | null>;
    readonly?: boolean;
  } = $props();

  let host: HTMLDivElement;
  let view: EditorView;

  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: '15px', backgroundColor: 'var(--surface)', color: 'var(--text)' },
    '.cm-content': { fontFamily: 'var(--mono)', padding: '16px 0', caretColor: 'var(--accent)' },
    '.cm-line': { padding: '0 18px' },
    '.cm-gutters': { backgroundColor: 'var(--surface)', borderRight: '1px solid var(--border)', color: 'var(--muted)' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--sunken)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--accent-soft) !important' },
    '.cm-cursor': { borderLeftColor: 'var(--accent)' },
  });

  async function dropFiles(files: File[], pos: number) {
    if (!onfiles || !files.length) return false;
    const text = await onfiles(files);
    if (text) view.dispatch({ changes: { from: pos, insert: text + '\n' } });
    return true;
  }

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          basicSetup,
          markdown(),
          EditorView.lineWrapping,
          theme,
          EditorState.readOnly.of(readonly),
          keymap.of([indentWithTab, { key: 'Mod-s', preventDefault: true, run: () => { onsave?.(); return true; } }]),
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

  export function insert(text: string) {
    const pos = view.state.selection.main.head;
    view.dispatch({ changes: { from: pos, insert: text }, selection: { anchor: pos + text.length } });
    view.focus();
  }
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor { height: 100%; min-height: 300px; border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; }
  .editor :global(.cm-editor) { height: 100%; }
  .editor :global(.cm-scroller) { overflow: auto; }
</style>
