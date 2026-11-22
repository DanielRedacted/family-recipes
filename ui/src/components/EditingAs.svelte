<script lang="ts">
  // The desktop app: the person its changes are credited to (in recipes, the whiteboard and the change
  // log). Shown in the folder column and in Settings → People.
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { setEditingAs } from '../lib/ops';

  /** the People page passes its own list, so it follows adds and removals */
  let { people: given }: { people?: { id: string; name: string }[] | null } = $props();
  let loaded = $state<{ id: string; name: string }[]>([]);
  onMount(async () => { if (!given) try { loaded = await api('GET', '/api/users'); } catch {} });
  const people = $derived(given ?? loaded);
  const current = $derived(app.session.user && app.session.user.id !== 'admin' ? app.session.user.id : '');
</script>

{#if people.length}
  <label class="editing-as">Editing as
    <select class="plain-select" value={current} onchange={(e) => setEditingAs(e.currentTarget.value)}>
      <option value="">Choose…</option>
      {#each people as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
    </select>
  </label>
{/if}

<style>
  .editing-as { display: inline-flex; align-items: center; gap: 4px; font-size: 14px; color: var(--muted); min-height: 36px; }
</style>
