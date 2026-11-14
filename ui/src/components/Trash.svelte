<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { app, go, noteHash, toast, loadVault } from '../lib/state.svelte';
  import { confirmIt } from '../lib/dialog.svelte';

  let items = $state<{ id: string; path: string; deleted: number }[] | null>(null);
  const load = async () => { try { items = await api('GET', '/api/trash'); } catch (e: any) { toast(e.message, 'error'); items = []; } };
  onMount(load);

  async function restore(id: string) {
    try {
      const r = await api('POST', '/api/trash/restore', { id });
      await loadVault();
      await load();
      toast(`Restored ${r.path}`, 'ok');
      if (r.path.endsWith('.md')) go(noteHash(r.path));
    } catch (e: any) { toast(e.message, 'error'); }
  }

  async function empty() {
    if (!(await confirmIt('Empty the trash?', 'Deleted recipes and earlier versions are removed for good.', 'Empty trash', true))) return;
    try { await api('DELETE', '/api/trash'); await load(); toast('Trash emptied', 'ok'); } catch (e: any) { toast(e.message, 'error'); }
  }
</script>

<div class="page">
  <div class="row"><h1>Trash</h1><span class="spacer"></span>
    {#if app.session.role === 'admin' && items?.length}<button class="btn danger" onclick={empty}>Empty trash</button>{/if}
  </div>
  <p class="muted small">Deleted files stay here until the trash is emptied on the Mac. Earlier versions of edited recipes are kept too (in the hidden <code>.trash/versions</code> folder).</p>
  {#if items === null}<p class="muted">Loading…</p>
  {:else if !items.length}<p class="muted">The trash is empty.</p>
  {:else}
    <ul>
      {#each items as t (t.id)}
        <li class="row">
          <span class="grow">{t.path}</span>
          <span class="muted small">{new Date(t.deleted * 1000).toLocaleString()}</span>
          <button class="btn" onclick={() => restore(t.id)}>Restore</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .page { padding: 16px 28px 80px; max-width: 820px; }
  ul { list-style: none; padding: 0; }
  li { padding: 8px 0; border-bottom: 1px solid var(--border); }
  .grow { flex: 1; overflow-wrap: anywhere; }
  @media (max-width: 899px) { .page { padding: 8px 14px 96px; } }
</style>
