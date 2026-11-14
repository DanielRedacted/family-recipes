<script lang="ts">
  import { dialog } from '../lib/dialog.svelte';
  import { app, loadSession, toast } from '../lib/state.svelte';
  import { api } from '../lib/api';
  import { menu, closeMenu } from '../lib/menu.svelte';
  import IconPicker from './IconPicker.svelte';

  let value = $state('');
  let pin = $state('');
  let pinError = $state('');
  let busy = $state(false);

  $effect(() => {
    if (dialog.current) value = dialog.current.value ?? dialog.current.options?.[0] ?? '';
  });

  function submit(e: Event) {
    e.preventDefault();
    dialog.current?.resolve(dialog.current.value !== undefined || dialog.current.options ? value : 'ok');
  }

  async function unlock(e: Event) {
    e.preventDefault();
    busy = true;
    pinError = '';
    try {
      await api('POST', '/api/session/pin', { pin });
      await loadSession();
      app.pinPrompt = false;
      pin = '';
      toast('Editing unlocked on this device', 'ok');
    } catch (err: any) {
      pinError = err.message;
    } finally {
      busy = false;
    }
  }

  const focus = (el: HTMLElement, on: boolean = true) => { if (on) setTimeout(() => el.focus(), 0); };
</script>

{#if dialog.current}
  {@const d = dialog.current}
  <div class="modal-back" role="presentation" onclick={() => d.resolve(null)} onkeydown={(e) => e.key === 'Escape' && d.resolve(null)}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div role="dialog" aria-modal="true" aria-label={d.title} tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <form class="modal card" onsubmit={submit}>
      <h3>{d.title}</h3>
      {#if d.message}<p class="muted">{d.message}</p>{/if}
      {#if d.options}
        <select class="field" bind:value use:focus>
          {#each d.options as o}<option value={o}>{o || '(top level)'}</option>{/each}
        </select>
      {:else if d.value !== undefined}
        <input class="field" bind:value use:focus />
      {/if}
      <div class="row end">
        <button type="button" class="btn" onclick={() => d.resolve(null)}>Cancel</button>
        {#if d.alt}<button type="button" class="btn" onclick={() => d.resolve('alt')}>{d.alt}</button>{/if}
        <button type="submit" class="btn primary" class:danger-fill={d.danger} use:focus={d.value === undefined && !d.options}>{d.ok ?? 'OK'}</button>
      </div>
    </form>
    </div>
  </div>
{/if}

{#if app.pinPrompt}
  <div class="modal-back" role="presentation" onclick={() => (app.pinPrompt = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div role="dialog" aria-modal="true" aria-label="Enter PIN" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <form class="modal card" onsubmit={unlock}>
      <h3>Unlock editing</h3>
      <p class="muted small">Enter the PIN set in the Recipes app on the Mac. This device stays unlocked for 90 days.</p>
      <input class="field" type="password" inputmode="numeric" autocomplete="current-password" bind:value={pin} use:focus aria-label="PIN" />
      {#if pinError}<p class="error small">{pinError}</p>{/if}
      <div class="row end">
        <button type="button" class="btn" onclick={() => (app.pinPrompt = false)}>Cancel</button>
        <button class="btn primary" disabled={busy || !pin}>Unlock</button>
      </div>
    </form>
    </div>
  </div>
{/if}

{#if menu.items.length}
  <div class="menu-back" role="presentation" onclick={closeMenu} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }}></div>
  <div class="menu card" role="menu" style="left:{menu.x}px; top:{menu.y}px">
    {#each menu.items as it}
      {#if it.sep}<hr />{/if}
      <button role="menuitem" class:danger={it.danger} onclick={() => { closeMenu(); it.run(); }}>{it.label}</button>
    {/each}
  </div>
{/if}

<IconPicker />

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}">{t.text}</div>
  {/each}
</div>

<style>
  .modal-back { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.35); display: grid; place-items: center; z-index: 100; padding: 16px; }
  .modal { width: min(440px, calc(100vw - 32px)); padding: 18px 20px; display: grid; gap: 12px; }
  .modal h3 { margin: 0; }
  .modal p { margin: 0; }
  .end { justify-content: flex-end; }
  .error { color: var(--danger); }
  :global(.btn.primary.danger-fill) { background: var(--danger); border-color: var(--danger); }
  .menu-back { position: fixed; inset: 0; z-index: 90; }
  .menu { position: fixed; z-index: 91; min-width: 200px; padding: 4px; display: grid; }
  .menu button { text-align: left; background: none; border: 0; padding: 9px 12px; border-radius: 6px; cursor: pointer; }
  .menu button:hover { background: var(--sunken); }
  .menu button.danger { color: var(--danger); }
  .menu hr { border: 0; border-top: 1px solid var(--border); margin: 4px 6px; }
  .toasts { position: fixed; bottom: calc(16px + env(safe-area-inset-bottom)); left: 50%; transform: translateX(-50%); display: grid; gap: 8px; z-index: 200; pointer-events: none; }
  .toast { background: var(--text); color: var(--bg); padding: 10px 16px; border-radius: 8px; box-shadow: var(--shadow-md); max-width: 90vw; }
  .toast.error { background: var(--danger); color: #fff; }
  .toast.ok { background: var(--ok); color: #fff; }
  @media (max-width: 899px) { .toasts { bottom: calc(72px + env(safe-area-inset-bottom)); } }
</style>
