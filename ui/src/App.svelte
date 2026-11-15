<script lang="ts">
  import Icon from './components/Icon.svelte';
  import { onMount } from 'svelte';
  import { app, loadSession, loadVault, listenForChanges, go, canWrite } from './lib/state.svelte';
  import { newNote, newFolder, openExternal, plusMenu } from './lib/ops';
  import { back } from './lib/state.svelte';
  import Back from './components/Back.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import Folder from './components/Folder.svelte';
  import Note from './components/Note.svelte';
  import Settings from './components/Settings.svelte';
  import SettingsNav from './components/SettingsNav.svelte';
  import Import from './components/Import.svelte';
  import Ingredients from './components/Ingredients.svelte';
  import Switcher from './components/Switcher.svelte';
  import Overlays from './components/Overlays.svelte';

  onMount(async () => {
    await loadSession();
    await loadVault();
    listenForChanges();
  });

  const dir = $derived(app.route.view === 'folder' ? app.route.path : app.route.path.includes('/') ? app.route.path.slice(0, app.route.path.lastIndexOf('/')) : '');

  function key(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && (e.key === 'k' || e.key === 'o' || e.key === 'p')) { e.preventDefault(); app.switcher = !app.switcher; }
    else if (mod && e.key.toLowerCase() === 'n') { e.preventDefault(); if (e.shiftKey) newFolder(dir); else newNote(dir); }
    else if (mod && e.key === ',') { e.preventDefault(); go('#/settings'); }
    else if (mod && e.key === '[') { e.preventDefault(); back(); }
  }

  // Every link in the app works on a plain click: pages of this app open here, web pages in the
  // browser (from the desktop app through the system, so they never replace the app's window).
  function linkClick(e: MouseEvent) {
    if (e.defaultPrevented || e.button !== 0) return;
    const a = (e.target as Element | null)?.closest?.('a[href]') as HTMLAnchorElement | null;
    if (!a || a.closest('.cm-editor')) return;
    const href = a.getAttribute('href') ?? '';
    if (href.startsWith('#/')) {
      e.preventDefault();
      go(href);
    } else if (/^https?:\/\//i.test(href) && new URL(href).host !== location.host) {
      e.preventDefault();
      openExternal(href);
    }
  }
</script>

<svelte:window onkeydown={key} />
<svelte:document onclick={linkClick} />

<div class="app" class:open={app.sidebar}>
  <aside class="sidebar no-print" aria-label={app.route.view === 'settings' ? 'Settings' : 'Recipes folder'}>
    {#if app.route.view === 'settings'}<SettingsNav />{:else}<Sidebar />{/if}
  </aside>
  <div class="scrim no-print" role="presentation" onclick={() => (app.sidebar = false)}></div>
  <div class="main-col">
    <header class="mobilebar no-print">
      <button class="btn ghost icon" aria-label="Open folders" onclick={() => (app.sidebar = true)}><Icon name="menu" size={22} /></button>
      {#if app.canBack}<Back />{/if}
      <a href="#/" class="mtitle">Recipes</a>
      <button class="btn ghost icon" aria-label="Search" onclick={() => (app.switcher = true)}><Icon name="search" size={22} /></button>
    </header>
    <main>
      {#if app.route.view === 'note'}
        {#key app.route.path}<Note path={app.route.path} mode={app.route.mode} />{/key}
      {:else if app.route.view === 'settings'}<Settings />
      {:else if app.route.view === 'import'}<Import />
      {:else if app.route.view === 'ingredients'}<Ingredients query={app.route.path} />
      {:else}{#key app.route.path}<Folder path={app.route.path} />{/key}
      {/if}
    </main>
    <nav class="bottombar no-print" aria-label="Main">
      <button onclick={() => go('#/')} class:on={app.route.view === 'folder' && !app.route.path}><Icon name="home" size={22} />Home</button>
      <button onclick={() => (app.sidebar = true)}><Icon name="folder" size={22} />Folders</button>
      <button onclick={() => (app.switcher = true)}><Icon name="search" size={22} />Search</button>
      {#if canWrite()}<button onclick={(e) => plusMenu(e, dir)} class:on={app.route.view === 'import'}><Icon name="plus" size={22} />Add</button>{/if}
      <button onclick={() => go('#/settings')} class:on={app.route.view === 'settings'}><Icon name="settings" size={22} />Settings</button>
    </nav>
  </div>
</div>

<Switcher />
<Overlays />

<style>
  .app { display: grid; grid-template-columns: var(--sidebar) 1fr; grid-template-rows: minmax(0, 1fr); height: 100vh; height: 100dvh; overflow: hidden; }
  /* both columns are exactly one window tall; only their insides scroll */
  .sidebar { border-right: 1px solid var(--border); background: var(--sunken); overflow: hidden; min-height: 0; max-height: 100dvh; padding-top: env(safe-area-inset-top); }
  .main-col { display: flex; flex-direction: column; min-width: 0; min-height: 0; height: 100%; }
  main { flex: 1; overflow-y: auto; min-width: 0; }
  .mobilebar, .bottombar, .scrim { display: none; }
  @media (max-width: 899px) {
    .app { grid-template-columns: 1fr; }
    .sidebar {
      position: fixed; inset: 0 auto 0 0; width: min(86vw, 330px); z-index: 50; transform: translateX(-100%);
      transition: transform 0.2s ease; box-shadow: var(--shadow-md);
    }
    .open .sidebar { transform: none; }
    .open .scrim { display: block; position: fixed; inset: 0; background: rgba(0, 0, 0, 0.3); z-index: 49; }
    .mobilebar {
      display: flex; align-items: center; gap: 4px; padding: calc(4px + env(safe-area-inset-top)) 8px 4px;
      border-bottom: 1px solid var(--border); background: var(--bg);
    }
    .mtitle { flex: 1; text-align: center; font-family: var(--serif); font-weight: 700; font-size: 18px; color: var(--text); text-decoration: none; }
    .bottombar {
      display: flex; position: fixed; bottom: 0; left: 0; right: 0; z-index: 40; background: var(--surface);
      border-top: 1px solid var(--border); padding-bottom: env(safe-area-inset-bottom);
    }
    .bottombar button {
      flex: 1; display: grid; justify-items: center; gap: 1px; padding: 7px 0 6px; border: 0; background: none;
      font-size: 11px; color: var(--muted); cursor: pointer; min-height: 52px;
    }
    .bottombar button.on { color: var(--accent-ink); }
  }
</style>
