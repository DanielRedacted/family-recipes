<script lang="ts" module>
  // Which sections are folded, kept in this browser (every tab and page shares it): a list of ids.
  const KEY = 'closedSections';
  const closed = $state<Record<string, boolean>>((() => {
    try {
      const v = JSON.parse(localStorage.getItem(KEY) || '[]');
      return Array.isArray(v) ? Object.fromEntries(v.filter((x) => typeof x === 'string').map((x) => [x, true])) : {};
    } catch { return {}; }
  })());
  function setOpen(id: string, open: boolean) {
    if (open) delete closed[id];
    else closed[id] = true;
    try { localStorage.setItem(KEY, JSON.stringify(Object.keys(closed))); } catch {}
  }
</script>

<script lang="ts">
  // A page section whose heading folds it away. The heading stays a heading (a button inside it),
  // so screen readers can still move between sections by heading; a <summary> would hide it.
  import Icon from './Icon.svelte';
  import type { Snippet } from 'svelte';

  let { id, title, count, aside, children, class: cls = '' }: { id: string; title: string; count?: number; aside?: Snippet; children: Snippet; class?: string } = $props();
  const open = $derived(!closed[id]);

  /** Folded content stays findable with the browser's Find (hidden="until-found"; plain hidden in Safari). */
  function fold(el: HTMLElement, isOpen: boolean) {
    const set = (o: boolean) => (o ? el.removeAttribute('hidden') : el.setAttribute('hidden', 'until-found'));
    const found = () => setOpen(id, true);
    set(isOpen);
    el.addEventListener('beforematch', found);
    return { update: set, destroy: () => el.removeEventListener('beforematch', found) };
  }
</script>

<section class="sect {cls}" class:folded={!open}>
  <div class="head">
    <h2 class="sub">
      <button type="button" aria-expanded={open} onclick={() => setOpen(id, !open)}>
        <span class="chev" class:open aria-hidden="true"><Icon name="chevron-right" size={16} /></span>{title}{#if count}<span class="n">{count}</span>{/if}
      </button>
    </h2>
    {#if aside}{@render aside()}{/if}
  </div>
  <div use:fold={open}>{@render children()}</div>
</section>

<style>
  .sect { margin-top: 24px; }
  .sect:first-child { margin-top: 16px; } /* room under the toolbar row */
  /* folded sections stack closely, heading under heading */
  .folded + :global(.sect) { margin-top: 8px; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 8px; }
  .folded .head { margin-bottom: 0; }
  h2 { margin: 0; }
  /* the heading itself is the control: same text, a chevron before it, the full row's height to tap */
  button { display: inline-flex; align-items: center; gap: 4px; min-height: 36px; padding: 0 8px 0 0; border: 0; background: none; font: inherit; color: inherit; cursor: pointer; border-radius: 6px; }
  button:hover { color: var(--text); }
  .chev { display: inline-flex; transition: transform 0.1s; }
  .chev.open { transform: rotate(90deg); }
  .n { margin-left: 4px; font-size: 0.85em; font-variant-numeric: tabular-nums; }
  @media (prefers-reduced-motion: reduce) { .chev { transition: none; } }
  @media (pointer: coarse) { button { min-height: 40px; } }
</style>
