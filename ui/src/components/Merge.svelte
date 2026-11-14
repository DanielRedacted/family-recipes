<script lang="ts">
  import { onMount } from 'svelte';
  import { MergeView } from '@codemirror/merge';
  import { EditorView } from '@codemirror/view';
  import { EditorState } from '@codemirror/state';
  import { basicSetup } from 'codemirror';
  import { markdown } from '@codemirror/lang-markdown';
  import { yamlFrontmatter } from '@codemirror/lang-yaml';

  // Left: the version on disk (read-only). Right: the user's version (editable) — saved on accept.
  let { theirs, mine, leftLabel = 'Saved version', rightLabel = 'Your version', accept = 'Save my version', onaccept, oncancel }: {
    theirs: string; mine: string; leftLabel?: string; rightLabel?: string; accept?: string;
    onaccept: (text: string) => void; oncancel: () => void;
  } = $props();

  let host: HTMLDivElement;
  let mv: MergeView;
  onMount(() => {
    const ext = [basicSetup, yamlFrontmatter({ content: markdown() }), EditorView.lineWrapping];
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

<div class="back" role="presentation">
  <div class="box card" role="dialog" aria-modal="true" aria-label="Compare versions">
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
</div>

<style>
  .back { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.35); z-index: 100; display: grid; place-items: center; padding: 16px; }
  .box { width: min(1200px, 100%); height: min(86vh, 900px); display: flex; flex-direction: column; overflow: hidden; }
  .head, .foot { padding: 10px 14px; border-bottom: 1px solid var(--border); }
  .foot { border-top: 1px solid var(--border); border-bottom: 0; }
  .half { flex: 1; }
  .merge { flex: 1; overflow: auto; font-size: 14px; }
  .merge :global(.cm-mergeView) { height: 100%; }
  .merge :global(.cm-editor) { background: var(--surface); }
</style>
