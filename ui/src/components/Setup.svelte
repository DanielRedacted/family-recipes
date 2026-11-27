<script lang="ts">
  // First start: the basic settings on one page. Everything here can be changed later in Settings.
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { app, loadSession, toast } from '../lib/state.svelte';
  import { shell } from '../lib/ops';
  import Network from './Network.svelte';
  import GitHub from './GitHub.svelte';

  let s = $state<any>(null);
  let pin = $state('');
  let folder = $state(''); // a different folder was chosen: it takes effect after a restart
  let busy = $state(false);

  onMount(async () => {
    try { s = await api('GET', '/api/settings'); } catch (e: any) { toast(e.message, 'error'); }
  });

  async function put(body: any) {
    try { s = await api('PUT', '/api/settings', body); } catch (e: any) { toast(e.message, 'error'); }
  }

  async function chooseFolder() {
    const r = await shell('choose-vault');
    if (r?.path) { await put({ vault: r.path }); folder = r.path; }
  }

  async function done() {
    busy = true;
    try {
      if (pin.trim()) await api('PUT', '/api/settings/pin', { pin: pin.trim() });
      await api('PUT', '/api/settings', { setup_done: true });
      if (folder) await shell('restart');
      else await loadSession();
    } catch (e: any) {
      toast(e.message, 'error');
    } finally {
      busy = false;
    }
  }
</script>

<div class="setup" role="dialog" aria-modal="true" aria-labelledby="setup-title">
  <div class="box">
    <h1 id="setup-title">Set up Recipes</h1>
    <p class="muted">A few settings to start with. You can change all of them later in Settings.</p>
    {#if !s}
      <p class="muted">Loading…</p>
    {:else}
      <section class="card">
        <h2>Recipes folder</h2>
        <p class="mono">{folder || s.vault}</p>
        <p class="muted small">Every recipe is a plain text (Markdown) file in this folder, one subfolder per category. Choose the folder that holds your recipes, or keep this one and add recipes later.</p>
        {#if app.session.shell}<div class="row"><button class="btn" onclick={chooseFolder}>Use a different folder…</button></div>{/if}
      </section>
      <section class="card">
        <h2>Recipes from GitHub</h2>
        {#if folder}
          <p class="muted small">After the restart, connect the new folder in Settings → Recipes folder.</p>
        {:else}
          <GitHub status={s.git} onchange={(g) => (s.git = g)} />
        {/if}
      </section>
      {#if app.session.shell}
        <section class="card">
          <h2>This Mac</h2>
          <label class="row toggle"><input type="checkbox" checked={!!s.autostart} onchange={async (e) => { const r = await shell('autostart', { enabled: e.currentTarget.checked }); if (r) s.autostart = r.enabled; }} /> Open Recipes at login</label>
          <p class="muted small">Closing the window keeps Recipes running in the menu bar, so phones and tablets can still reach it.</p>
        </section>
      {/if}
      <section class="card">
        <h2>Phones, tablets and other computers</h2>
        <Network status={s.network} lan={s.lan} keepAwake={s.keep_awake} onchange={put} />
      </section>
      <section class="card">
        <h2>Family PIN</h2>
        <p class="muted small">Asked once on each phone, tablet or other computer when someone signs in with their name. With it, everyone signed in can edit, import and delete. Leave it empty to keep other devices read-only.</p>
        <input class="field" type="password" placeholder="Choose a PIN (optional)" bind:value={pin} autocomplete="new-password" aria-label="Family PIN" />
      </section>
      <div class="row finish">
        <button class="btn primary" onclick={done} disabled={busy}>{folder ? 'Done — restart Recipes' : 'Done'}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .setup { position: fixed; inset: 0; z-index: 60; overflow-y: auto; background: var(--bg); padding: calc(24px + env(safe-area-inset-top)) max(16px, env(safe-area-inset-right)) 40px max(16px, env(safe-area-inset-left)); }
  .box { max-width: 640px; margin: 0 auto; display: grid; gap: 16px; }
  h1 { margin: 0; }
  p { margin: 0; }
  section { padding: 16px 20px; display: grid; gap: 12px; }
  section h2 { margin: 0; font-size: 1.15rem; }
  .mono { font-family: var(--mono); font-size: 13px; overflow-wrap: anywhere; }
  .toggle { gap: 8px; font-weight: 500; }
  .finish { justify-content: flex-end; }
</style>
