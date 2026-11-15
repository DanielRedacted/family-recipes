<script lang="ts">
  import { folderHash } from '../lib/state.svelte';
  import Back from './Back.svelte';

  /** Full trail from "All recipes" to `dir`, then the optional `leaf` (the open note). */
  let { dir, leaf }: { dir: string; leaf?: string } = $props();
  const segs = $derived(dir ? dir.split('/').map((name, i, a) => ({ name, path: a.slice(0, i + 1).join('/') })) : []);
</script>

<nav class="crumbs" aria-label="Breadcrumb">
  <Back />
  <a href="#/">All recipes</a>
  {#each segs as c}<span class="sep">›</span><a href={folderHash(c.path)}>{c.name}</a>{/each}
  {#if leaf}<span class="sep">›</span><span class="leaf">{leaf}</span>{/if}
</nav>

<style>
  .crumbs { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; min-width: 0; color: var(--muted); }
  .crumbs a { color: var(--muted); text-decoration: none; }
  .crumbs a:hover { color: var(--text); }
  .crumbs > :last-child { color: var(--text); }
  .leaf { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  @media (max-width: 899px) { .crumbs { display: none; } }
</style>
