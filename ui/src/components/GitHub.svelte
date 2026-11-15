<script lang="ts">
  // Connect the recipes folder to a GitHub repository once; after that every save is uploaded and
  // other Macs' changes are downloaded (crates/core/src/git.rs). Used by Setup and Settings.
  import { api } from '../lib/api';
  import { toast } from '../lib/state.svelte';
  import { confirmIt } from '../lib/dialog.svelte';

  let { status, onchange }: { status: any; onchange: (st: any) => void } = $props();
  let repo = $state('');
  let token = $state('');
  let busy = $state(false);

  async function call(fn: () => Promise<any>) {
    busy = true;
    try { onchange(await fn()); } catch (e: any) { toast(e.message, 'error'); } finally { busy = false; }
  }

  const connect = () => call(async () => {
    const st = await api('POST', '/api/git/connect', { url: repo, token });
    token = '';
    toast(`Connected to ${st.remote}`, 'ok');
    return st;
  });

  const syncNow = () => call(async () => {
    const st = await api('POST', '/api/git/sync');
    if (st.error) toast(st.error, 'error');
    else toast(st.pulled ? `Got ${st.pulled} ${st.pulled === 1 ? 'change' : 'changes'} from GitHub` : 'Up to date', 'ok');
    return st;
  });

  async function disconnect() {
    if (!(await confirmIt('Stop syncing with GitHub?', 'The recipes stay in this folder, but changes are no longer uploaded or downloaded.', 'Stop syncing', true))) return;
    call(() => api('DELETE', '/api/git'));
  }

  const ago = (t: number) => {
    const m = Math.round((Date.now() / 1000 - t) / 60);
    return m < 1 ? 'just now' : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} hr ago` : new Date(t * 1000).toLocaleDateString();
  };
</script>

{#if status?.connected}
  <p>Synced with <b>{status.remote}</b>{#if status.last_sync} · {ago(status.last_sync)}{/if}</p>
  {#if status.error}<p class="err small">{status.error}</p>{/if}
  {#if status.waiting}<p class="muted small">{status.waiting} {status.waiting === 1 ? 'change' : 'changes'} waiting to upload.</p>{/if}
  <p class="muted small">Every save is uploaded to GitHub, and changes made on other Macs are downloaded when Recipes opens and every few minutes. If two Macs change the same recipe at once, both versions are kept: the other one is saved next to it as “… (from other Mac)”.</p>
  <div class="row">
    <button class="btn" onclick={syncNow} disabled={busy}>{busy ? 'Checking…' : 'Sync now'}</button>
    <button class="btn danger" onclick={disconnect} disabled={busy}>Stop syncing</button>
  </div>
{:else}
  <p class="muted small">Optional. Keep the recipes in a private GitHub repository, shared between Macs. Connect once and every save is uploaded automatically. The recipes folder must be empty or already a copy of that repository.</p>
  <form class="grid" onsubmit={(e) => { e.preventDefault(); connect(); }}>
    <label>Repository
      <input class="field" bind:value={repo} placeholder="owner/name" autocomplete="off" autocapitalize="off" spellcheck="false" />
    </label>
    <label>Access token
      <input class="field" type="password" bind:value={token} placeholder="github_pat_…" autocomplete="off" />
    </label>
    <p class="muted small">The repository’s owner creates a <a href="https://github.com/settings/personal-access-tokens/new">fine-grained token on GitHub</a>: <i>Only select repositories</i> → the recipes repository, <i>Permissions</i> → <i>Contents: Read and write</i>. The same token works on every family Mac. It’s kept in the macOS Keychain.</p>
    <div class="row"><button class="btn primary" disabled={busy || !repo.trim()}>{busy ? 'Connecting…' : 'Connect'}</button></div>
  </form>
{/if}

<style>
  p { margin: 0; }
  .grid { display: grid; gap: 12px; }
  .grid label { display: grid; gap: 4px; font-size: 14px; }
  .err { color: var(--danger); }
</style>
