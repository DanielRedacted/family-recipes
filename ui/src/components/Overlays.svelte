<script lang="ts">
  import { dialog } from '../lib/dialog.svelte';
  import { app } from '../lib/state.svelte';
  import { menu, closeMenu } from '../lib/menu.svelte';
  import { modal } from '../lib/modal';
  import IconPicker from './IconPicker.svelte';

  let value = $state('');

  let checked = $state(false);
  $effect(() => {
    if (dialog.current) { value = dialog.current.value ?? dialog.current.options?.[0] ?? ''; checked = !!dialog.current.checked; }
  });

  function submit(e: Event) {
    e.preventDefault();
    dialog.current?.resolve(dialog.current.value !== undefined || dialog.current.options ? value : 'ok', checked);
  }

  const focus = (el: HTMLElement, on: boolean = true) => { if (on) setTimeout(() => el.focus(), 0); };

  // The menu from the keyboard: arrows, Home and End move through it; Esc and Tab close it and go back
  // to what opened it. (Not a popover: that needs Safari 17, newer than macOS 12's.)
  let menuEl: HTMLElement | undefined = $state();
  $effect(() => { if (menu.items.length && menu.kbd) menuEl?.querySelector('button')?.focus(); });
  function menuKey(e: KeyboardEvent) {
    if (!menu.items.length || !menuEl) return;
    if (e.key === 'Escape' || e.key === 'Tab') { e.preventDefault(); closeMenu(true); return; }
    const items = [...menuEl.querySelectorAll('button')];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const to = ({ ArrowDown: i + 1, ArrowUp: i < 0 ? -1 : i - 1, Home: 0, End: -1 } as Record<string, number>)[e.key];
    if (to === undefined) return;
    e.preventDefault();
    items[(to + items.length) % items.length].focus();
  }
</script>

<svelte:window onkeydown={menuKey} />

{#if dialog.current}
  {@const d = dialog.current}
  <dialog class="modal-host" aria-labelledby="ask-title" use:modal={{ onclose: () => dialog.current?.resolve(null), outside: true }}>
    <form class="modal card" onsubmit={submit}>
      <h3 id="ask-title">{d.title}</h3>
      {#if d.message}<p class="muted">{d.message}</p>{/if}
      {#if d.options}
        <select class="field" bind:value use:focus>
          {#each d.options as o}<option value={o}>{o || '(top level)'}</option>{/each}
        </select>
      {:else if d.value !== undefined}
        <input class="field" bind:value use:focus placeholder={d.placeholder} aria-label={d.placeholder ?? d.title} />
      {/if}
      {#if d.check}<label class="row check"><input type="checkbox" bind:checked /> {d.check}</label>{/if}
      <div class="row end">
        <button type="button" class="btn" onclick={() => d.resolve(null)}>Cancel</button>
        {#if d.alt}<button type="button" class="btn" onclick={() => d.resolve('alt')}>{d.alt}</button>{/if}
        <button type="submit" class="btn primary" class:danger-fill={d.danger} use:focus={d.value === undefined && !d.options}>{d.ok ?? 'OK'}</button>
      </div>
    </form>
  </dialog>
{/if}

{#if menu.items.length}
  <div class="menu-back" role="presentation" onclick={() => closeMenu()} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }}></div>
  <div class="menu card" role="menu" bind:this={menuEl} style="left:{menu.x}px; top:{menu.y}px">
    {#each menu.items as it}
      {#if it.sep}<hr />{/if}
      <button role="menuitem" class:danger={it.danger} onclick={(e) => { closeMenu(true, e.detail === 0); it.run(); }}>{it.label}</button>
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
  .modal { width: min(440px, calc(100vw - 32px)); padding: 20px; display: grid; gap: 12px; }
  .modal h3 { margin: 0; }
  .modal p { margin: 0; }
  .end { justify-content: flex-end; }
  .check { gap: 8px; flex-wrap: nowrap; min-height: 36px; }
  :global(.btn.primary.danger-fill) { background: var(--danger); border-color: var(--danger); }
  .menu-back { position: fixed; inset: 0; z-index: 90; }
  .menu { position: fixed; z-index: 91; min-width: 200px; padding: 4px; display: grid; }
  .menu button { text-align: left; background: none; border: 0; padding: 8px 12px; border-radius: 6px; cursor: pointer; }
  .menu button:hover { background: var(--sunken); }
  .menu button.danger { color: var(--danger); }
  .menu hr { border: 0; border-top: 1px solid var(--border); margin: 4px 8px; }
  .toasts { position: fixed; bottom: calc(16px + env(safe-area-inset-bottom)); left: 50%; transform: translateX(-50%); display: grid; gap: 8px; z-index: 200; pointer-events: none; }
  .toast { background: var(--text); color: var(--bg); padding: 12px 16px; border-radius: 8px; box-shadow: var(--shadow-md); max-width: 90vw; }
  .toast.error { background: var(--danger); color: #fff; }
  .toast.ok { background: var(--ok); color: #1b1a22; } /* dark text on green: 5.2:1 */
  @media (max-width: 899px) { .toasts { bottom: calc(72px + env(safe-area-inset-bottom)); } }
</style>
