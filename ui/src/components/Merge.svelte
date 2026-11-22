<script lang="ts">
  import { onMount, getContext } from 'svelte';
  import { app, type Tab } from '../lib/state.svelte';
  import { modal } from '../lib/modal';
  import { MergeView } from '@codemirror/merge';
  import { EditorView } from '@codemirror/view';
  import { EditorState } from '@codemirror/state';
  import { basicSetup } from 'codemirror';
  import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
  import { yamlFrontmatter } from '@codemirror/lang-yaml';

  // Left: the version on disk (read-only). Right: the user's version (editable) — saved on accept.
  let { theirs, mine, leftLabel = 'Saved version', rightLabel = 'Your version', accept = 'Save my version', onaccept, oncancel }: {
    theirs: string; mine: string; leftLabel?: string; rightLabel?: string; accept?: string;
    onaccept: (text: string) => void; oncancel: () => void;
  } = $props();
  const tab = getContext<Tab>('tab'); // hidden, not closed, while another tab shows

  let host: HTMLDivElement;
  let mv: MergeView;
  onMount(() => {
    const ext = [basicSetup, yamlFrontmatter({ content: markdown({ base: markdownLanguage }) }), EditorView.lineWrapping];
    mv = new MergeView({
      parent: host,
      a: { doc: theirs, extensions: [...ext, EditorState.readOnly.of(true)] },
      b: { doc: mine, extensions: ext },
      revertControls: 'a-to-b',
      highlightChanges: true,
    });
    return () => mv.destroy();
  });
</script>

<dialog class="modal-host" aria-label="Compare versions" use:modal={{ onclose: oncancel, active: tab.id === app.tab }}>
  <div class="box card">
    <div class="row head">
      <strong class="half">{leftLabel}</strong><strong class="half">{rightLabel}</strong>
    </div>
    <div class="merge" bind:this={host}></div>
    <div class="row foot">
      <span class="muted small">Arrows copy a change from left to right. Edit the right side freely.</span>
      <span class="spacer"></span>
      <button class="btn" onclick={oncancel}>Cancel</button>
      <button class="btn primary" onclick={() => onaccept(mv.b.state.doc.toString())}>{accept}</button>
    </div>
  </div>
</dialog>

<style>
  .box { width: min(1200px, calc(100vw - 32px)); height: min(86vh, 900px); display: flex; flex-direction: column; overflow: hidden; }
  .head, .foot { padding: 12px 16px; border-bottom: 1px solid var(--border); }
  .foot { border-top: 1px solid var(--border); border-bottom: 0; }
  .half { flex: 1; }
  .merge { flex: 1; overflow: auto; font-size: 14px; }
  .merge :global(.cm-mergeView) { height: 100%; }
  .merge :global(.cm-editor) { background: var(--surface); }
</style>
