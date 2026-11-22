<script lang="ts">
  // Every phone, tablet or other computer signs in once as a person (a name, no password), with the
  // family PIN when one is set. The device stays signed in; Settings → General signs it out.
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { app, loadSession } from '../lib/state.svelte';

  let people = $state<{ id: string; name: string }[]>([]);
  let name = $state('');
  let fullName = $state('');
  let pin = $state('');
  let error = $state('');
  let busy = $state(false);

  onMount(async () => {
    try { people = await api('GET', '/api/users'); } catch { /* offline: a new name still works later */ }
  });

  async function signIn(who: string, full_name = '') {
    error = '';
    if (app.session.pin_set && !pin) { error = 'Enter the family PIN first.'; return; }
    busy = true;
    try {
      await api('POST', '/api/session', { name: who, full_name, pin });
      await loadSession();
    } catch (e: any) { error = e.message; } finally { busy = false; }
  }
</script>

<div class="signin" role="dialog" aria-modal="true" aria-labelledby="signin-title">
  <div class="box">
    <img src="/icon.svg" alt="" width="48" height="48" />
    <h1 id="signin-title">Who's cooking?</h1>
    <p class="muted">Pick your name or add yourself. This device remembers you.</p>
    {#if app.session.pin_set}
      <label class="pin">Family PIN
        <input class="field" type="password" inputmode="numeric" autocomplete="current-password" bind:value={pin} />
      </label>
    {/if}
    {#if people.length}
      <div class="names" role="group" aria-label="People">
        {#each people as p (p.id)}<button class="btn" disabled={busy} onclick={() => signIn(p.name)}>{p.name}</button>{/each}
      </div>
    {/if}
    <form class="add" onsubmit={(e) => { e.preventDefault(); signIn(name, fullName); }}>
      {#if people.length}<p class="small muted">Someone new?</p>{/if}
      <input class="field" placeholder="Your name, e.g. Ann" maxlength="40" autocomplete="nickname" bind:value={name} aria-label="Your name" />
      <input class="field" placeholder="Full name, for recipe credits" maxlength="80" autocomplete="name" bind:value={fullName} aria-label="Full name" />
      <button class="btn primary" disabled={busy || !name.trim() || !fullName.trim()}>{people.length ? 'Add me' : 'Start'}</button>
    </form>
    {#if error}<p class="error small" role="alert">{error}</p>{/if}
  </div>
</div>

<style>
  .signin { position: fixed; inset: 0; z-index: 60; overflow-y: auto; background: var(--bg); padding: calc(48px + env(safe-area-inset-top)) max(16px, env(safe-area-inset-right)) 40px max(16px, env(safe-area-inset-left)); }
  .box { max-width: 420px; margin: 0 auto; display: grid; gap: 16px; justify-items: start; }
  h1 { margin: 0; font-family: var(--serif); }
  p { margin: 0; }
  .pin { display: grid; gap: 4px; width: 100%; font-weight: 500; }
  .names { display: flex; flex-wrap: wrap; gap: 8px; }
  .names .btn { min-height: 40px; }
  .add { width: 100%; display: grid; gap: 8px; justify-items: start; }
  .add .field { width: 100%; }
  .error { color: var(--danger); }
</style>
