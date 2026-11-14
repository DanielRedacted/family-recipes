<script lang="ts">
  import { onMount } from 'svelte';
  import { app, toast, loadSession, loadVault } from '../lib/state.svelte';
  import { api } from '../lib/api';
  import { shell } from '../lib/ops';
  import { confirmIt } from '../lib/dialog.svelte';
  import Network from './Network.svelte';

  let theme = $state((() => { try { return localStorage.getItem('theme') || 'system'; } catch { return 'system'; } })());
  let s = $state<any>(null);
  let pin = $state('');
  let aiKey = $state('');

  $effect(() => {
    try { localStorage.setItem('theme', theme); } catch {}
    if (theme === 'system') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.setAttribute('data-theme', theme);
  });

  const admin = $derived(app.session.role === 'admin');

  async function refresh() { if (admin) s = await api('GET', '/api/settings'); }
  onMount(refresh);
  $effect(() => { if (admin && !s) refresh(); });

  async function put(body: any) {
    try { s = await api('PUT', '/api/settings', body); await loadSession(); } catch (e: any) { toast(e.message, 'error'); }
  }

  async function savePin(clear = false) {
    if (clear && !(await confirmIt('Remove the PIN?', 'Other devices will no longer be able to unlock editing.', 'Remove PIN', true))) return;
    try {
      await api('PUT', '/api/settings/pin', { pin: clear ? '' : pin });
      pin = '';
      toast(clear ? 'PIN removed' : 'PIN saved — all devices must enter it again', 'ok');
      await refresh();
      await loadSession();
    } catch (e: any) { toast(e.message, 'error'); }
  }

  async function chooseVault() {
    const r = await shell('choose-vault');
    if (r?.path) {
      if (!(await confirmIt('Use this folder for recipes?', r.path + ' — the app restarts its file watcher on that folder.', 'Use folder'))) return;
      await put({ vault: r.path });
      toast('Restart Recipes to finish switching folders.', 'info');
    }
  }

  async function saveKey() {
    try { await api('PUT', '/api/settings/ai-key', { key: aiKey }); aiKey = ''; toast('API key saved in the macOS Keychain', 'ok'); await refresh(); }
    catch (e: any) { toast(e.message, 'error'); }
  }

  async function lockDevice() { await api('DELETE', '/api/session'); await loadSession(); toast('This device is read-only again'); }
</script>

<div class="page">
  <h1>Settings</h1>

  <section class="card">
    <h2>Appearance</h2>
    <label class="row">Theme
      <select class="field w" bind:value={theme}>
        <option value="system">Match system</option><option value="light">Light</option><option value="dark">Dark</option>
      </select>
    </label>
  </section>

  {#if !admin}
    <section class="card">
      <h2>This device</h2>
      {#if app.session.role === 'writer'}
        <p>Editing is unlocked on this device.</p>
        <button class="btn" onclick={lockDevice}>Lock this device</button>
      {:else if app.session.pin_set}
        <p>Read-only. Enter the PIN from the Recipes app on the Mac to edit, import and delete.</p>
        <button class="btn primary" onclick={() => (app.pinPrompt = true)}>Unlock editing</button>
      {:else}
        <p>Read-only. To allow editing from this device, set a PIN in the Recipes app on the Mac.</p>
      {/if}
    </section>
  {:else if s}
    <section class="card">
      <h2>Recipes folder</h2>
      <p class="mono">{s.vault}</p>
      <p class="muted small">Every recipe is a plain text (Markdown) file in this folder. You can also open and edit them with any text editor or Obsidian; changes show up here instantly.</p>
      <div class="row">
        {#if app.session.shell}
          <button class="btn" onclick={() => shell('reveal', { path: '' })}>Show in Finder</button>
          <button class="btn" onclick={chooseVault}>Use a different folder…</button>
        {/if}
        <a class="btn" href="/api/download">Download everything (zip)</a>
      </div>
    </section>

    <section class="card">
      <h2>Phones, tablets and other computers</h2>
      <Network status={s.network} lan={s.lan} keepAwake={s.keep_awake} onchange={put} />
      <h3>Editing PIN</h3>
      <p class="muted small">Other devices can always read recipes. With the PIN they can also edit, import and delete. {s.pin_set ? 'A PIN is set.' : 'No PIN is set, so other devices are read-only.'}</p>
      <form class="row" onsubmit={(e) => { e.preventDefault(); savePin(); }}>
        <input class="field w" type="password" placeholder={s.pin_set ? 'New PIN' : 'Choose a PIN (6+ characters)'} bind:value={pin} autocomplete="new-password" aria-label="PIN" />
        <button class="btn primary" disabled={pin.trim().length < 6}>{s.pin_set ? 'Change PIN' : 'Set PIN'}</button>
        {#if s.pin_set}<button type="button" class="btn danger" onclick={() => savePin(true)}>Remove PIN</button>{/if}
      </form>
    </section>

    <section class="card">
      <h2>AI assistant <span class="muted small">(optional)</span></h2>
      <label class="row toggle"><input type="checkbox" checked={s.ai.enabled} onchange={(e) => put({ ai: { ...s.ai, enabled: e.currentTarget.checked } })} /> Enable AI features</label>
      <p class="muted small">Off by default. When on, AI can help fix imported recipes and answer questions about a recipe — always as a suggestion you accept or reject. Uses your own API key; nothing runs on this Mac.</p>
      {#if s.ai.enabled}
        <div class="grid2">
          <label>Provider
            <select class="field" value={s.ai.provider} onchange={(e) => put({ ai: { ...s.ai, provider: e.currentTarget.value } })}>
              <option value="anthropic">Anthropic (Claude)</option>
              <option value="openai">OpenAI-compatible</option>
            </select>
          </label>
          <label>Model
            <input class="field" value={s.ai.model} placeholder={s.ai.provider === 'anthropic' ? 'claude-opus-5-5' : 'gpt-…'} onchange={(e) => put({ ai: { ...s.ai, model: e.currentTarget.value } })} />
          </label>
          {#if s.ai.provider === 'openai'}
            <label>API base URL
              <input class="field" value={s.ai.base_url} placeholder="https://api.openai.com/v1" onchange={(e) => put({ ai: { ...s.ai, base_url: e.currentTarget.value } })} />
            </label>
          {/if}
          <form class="keyrow" onsubmit={(e) => { e.preventDefault(); saveKey(); }}>
            <label>API key {s.ai_key_set ? '(saved)' : ''}
              <input class="field" type="password" bind:value={aiKey} placeholder={s.ai_key_set ? '••••••••  (enter a new key to replace)' : 'Paste your API key'} autocomplete="off" />
            </label>
            <button class="btn" disabled={!aiKey.trim()}>Save key</button>
          </form>
        </div>
      {/if}
    </section>
  {:else}
    <p class="muted">Loading…</p>
  {/if}

  <p class="muted small">Recipes {app.session.version} · {app.files.filter((f) => f.kind === 'note').length} notes</p>
</div>

<style>
  .page { padding: 16px 28px 80px; max-width: 760px; display: grid; gap: 16px; }
  h1 { margin: 6px 0 0; }
  section { padding: 16px 18px; display: grid; gap: 10px; }
  section h2 { margin: 0; font-size: 1.15rem; }
  section h3 { margin: 10px 0 0; font-size: 1rem; }
  p { margin: 0; }
  .w { width: auto; min-width: 220px; }
  .mono { font-family: var(--mono); font-size: 13px; overflow-wrap: anywhere; }
  .toggle { gap: 10px; font-weight: 500; }
  .grid2 { display: grid; gap: 10px; }
  .grid2 label { display: grid; gap: 4px; font-size: 14px; }
  .keyrow { display: flex; gap: 8px; align-items: end; }
  .keyrow label { flex: 1; }
  @media (max-width: 899px) { .page { padding: 8px 14px 96px; } }
</style>
