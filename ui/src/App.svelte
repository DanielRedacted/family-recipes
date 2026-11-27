<script lang="ts">
  import Icon from './components/Icon.svelte';
  import { onMount } from 'svelte';
  import { app, loadSession, loadVault, listenForChanges, go, newTab, selectTab, wide } from './lib/state.svelte';
  import { newNote, newFolder, openExternal } from './lib/ops';
  import { back } from './lib/state.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import SettingsNav from './components/SettingsNav.svelte';
  import Pane from './components/Pane.svelte';
  import Tabs from './components/Tabs.svelte';
  import Switcher, { openSearch } from './components/Switcher.svelte';
  import Overlays from './components/Overlays.svelte';
  import Feedback, { openFeedback } from './components/Feedback.svelte';
  import Setup from './components/Setup.svelte';
  import SignIn from './components/SignIn.svelte';
  import { update, checkForUpdate, installUpdate, autoInstall } from './lib/update.svelte';

  // the desktop app's window: the page runs up to its title bar (app.css, html.shell)
  $effect(() => { document.documentElement.classList.toggle('shell', !!app.session.shell); });

  onMount(async () => {
    await loadSession();
    await loadVault();
    listenForChanges();
    if (app.session.shell) {
      setTimeout(() => checkForUpdate(true), 3000);
      document.addEventListener('visibilitychange', autoInstall); // the window was closed to the menu bar
    }
  });

  const dir = $derived(app.route.view === 'folder' ? app.route.path : app.route.path.includes('/') ? app.route.path.slice(0, app.route.path.lastIndexOf('/')) : '');

  function key(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && (e.key === 'k' || e.key === 'o' || e.key === 'p')) { e.preventDefault(); if (app.switcher) app.switcher = false; else openSearch(); }
    else if (mod && e.key.toLowerCase() === 'n') { e.preventDefault(); if (e.shiftKey) newFolder(dir); else newNote(dir); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'f') { e.preventDefault(); openFeedback(); }
    else if (mod && e.key === ',') { e.preventDefault(); go('#/settings'); }
    else if (mod && e.key === '[') { e.preventDefault(); back(); }
    else if (mod && e.key.toLowerCase() === 't' && wide()) { e.preventDefault(); newTab(); }
    else if (e.ctrlKey && e.key === 'Tab' && app.tabs.length > 1) {
      e.preventDefault();
      const i = app.tabs.findIndex((t) => t.id === app.tab);
      selectTab(app.tabs[(i + (e.shiftKey ? -1 : 1) + app.tabs.length) % app.tabs.length].id);
    }
  }

  // Every link in the app works on a plain click: pages of this app open here, web pages in the
  // browser (from the desktop app through the system, so they never replace the app's window).
  // ⌘-click or a middle click opens a page of this app in a new tab
  function linkClick(e: MouseEvent) {
    if (e.defaultPrevented || e.button > 1) return;
    const a = (e.target as Element | null)?.closest?.('a[href]') as HTMLAnchorElement | null;
    if (!a || a.closest('.cm-editor')) return;
    const href = a.getAttribute('href') ?? '';
    if (href.startsWith('#/')) {
      e.preventDefault();
      if ((e.button === 1 || e.metaKey || e.ctrlKey) && wide()) newTab(href);
      else if (e.button === 0) go(href);
    } else if (e.button === 0 && /^https?:\/\//i.test(href) && new URL(href).host !== location.host) {
      e.preventDefault();
      openExternal(href);
    }
  }
</script>

<svelte:window onkeydown={key} />
<svelte:document onclick={linkClick} onauxclick={linkClick} />

<div class="app" class:open={app.sidebar}>
  <aside class="sidebar no-print" aria-label={app.route.view === 'settings' ? 'Settings' : 'Recipes folder'}>
    {#if app.route.view === 'settings'}<SettingsNav />{:else}<Sidebar />{/if}
  </aside>
  <div class="scrim no-print" role="presentation" onclick={() => (app.sidebar = false)}></div>
  <div class="main-col">
    {#if update.version && !update.hidden}
      <div class="update no-print" role="status">
        <div class="utext">
          <strong>Recipes {update.version} is {update.ready ? 'ready' : 'available'}</strong>
          {#if update.notes}<details><summary>What's new</summary><p>{update.notes}</p></details>{/if}
        </div>
        <button class="btn" onclick={() => (update.hidden = true)} disabled={update.busy}>Later</button>
        <button class="btn primary" onclick={installUpdate} disabled={update.busy}>{update.busy ? 'Updating…' : update.ready ? 'Restart to update' : 'Update and restart'}</button>
      </div>
    {/if}
    <Tabs />
    <main>
      {#each app.tabs as t (t.id)}<Pane tab={t} />{/each}
    </main>
    <nav class="bottombar no-print" aria-label="Main">
      <button onclick={() => (app.sidebar = true)}><Icon name="menu" size={22} />Folders</button>
      <button onclick={() => go('#/')} class:on={app.route.view === 'folder' && !app.route.path}><Icon name="book" size={22} />Home</button>
      <button onclick={openSearch}><Icon name="search" size={22} />Search</button>
    </nav>
  </div>
</div>

<Switcher />
{#if app.session.setup}<Setup />{/if}
<!-- every device but the desktop app (always "Admin") signs in; not offline before any session was saved -->
{#if app.session.version && !app.session.user}<SignIn />{:else if app.session.books?.length === 0}
  <!-- every cookbook is Mac only (Settings → Cookbooks) -->
  <div class="nobook" role="status"><p>There's no cookbook to show here. On the Mac, open Settings → Cookbooks and turn off “Only on this Mac” for one.</p></div>
{/if}
<Overlays />
<Feedback />

<style>
  .nobook { position: fixed; inset: 0; z-index: 55; display: grid; place-items: center; padding: 16px; background: var(--bg); text-align: center; }
  .nobook p { max-width: 36ch; margin: 0; }
  .app { display: grid; grid-template-columns: var(--sidebar) 1fr; grid-template-rows: minmax(0, 1fr); height: 100vh; height: 100dvh; overflow: hidden; }
  /* landscape iPhone: keep clear of the Dynamic Island (0 everywhere else) */
  .app { padding-left: env(safe-area-inset-left); padding-right: env(safe-area-inset-right); }
  /* both columns are exactly one window tall; only their insides scroll */
  .sidebar { border-right: 1px solid var(--border); background: var(--sunken); overflow: hidden; min-height: 0; max-height: 100dvh; padding-top: env(safe-area-inset-top); }
  .main-col { display: flex; flex-direction: column; min-width: 0; min-height: 0; height: 100%; }
  /* each tab's page fills it and scrolls by itself (Pane.svelte) */
  main { flex: 1; position: relative; min-width: 0; min-height: 0; }
  .update {
    display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 12px 16px;
    background: var(--accent-soft); border-bottom: 1px solid var(--border);
  }
  .utext { flex: 1 1 240px; }
  .update summary { cursor: pointer; color: var(--accent-ink); font-size: 14px; }
  .update details p { margin: 8px 0 0; white-space: pre-line; font-size: 14px; }
  .bottombar, .scrim { display: none; }
  @media (max-width: 899px) {
    .app { grid-template-columns: 1fr; }
    .sidebar {
      position: fixed; inset: 0 auto 0 0; width: min(86vw, 330px); z-index: 50; transform: translateX(-100%);
      transition: transform 0.2s ease; box-shadow: var(--shadow-md); padding-left: env(safe-area-inset-left);
    }
    .open .sidebar { transform: none; }
    .open .scrim { display: block; position: fixed; inset: 0; background: rgba(0, 0, 0, 0.3); z-index: 49; }
    .main-col { padding-top: env(safe-area-inset-top); }
    .bottombar {
      display: flex; position: fixed; bottom: 0; left: 0; right: 0; z-index: 40; background: var(--surface);
      border-top: 1px solid var(--border); padding: 0 env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
    }
    .bottombar button {
      flex: 1; display: grid; justify-items: center; gap: 2px; padding: 8px 0; border: 0; background: none;
      font-size: 11px; color: var(--muted); cursor: pointer; min-height: 52px;
    }
    .bottombar button.on { color: var(--accent-ink); }
  }
</style>
