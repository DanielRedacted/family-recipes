<script lang="ts" module>
  import { app, canWrite } from '../lib/state.svelte';

  /** Settings pages, in order; each is shown only where it applies. */
  export function settingsSections(): { id: string; label: string; icon: string }[] {
    const admin = app.session.role === 'admin';
    return [
      { id: 'general', label: 'General', icon: 'settings', on: true },
      { id: 'folder', label: 'Recipes folder', icon: 'folder', on: admin },
      { id: 'app', label: 'Desktop app', icon: 'home', on: admin && app.session.shell },
      { id: 'devices', label: 'Phones & tablets', icon: 'lock', on: admin },
      { id: 'ai', label: 'AI assistant', icon: 'chef', on: admin },
      { id: 'recent', label: 'Recent changes', icon: 'clock', on: true },
      { id: 'trash', label: 'Trash', icon: 'trash', on: canWrite() },
    ].filter((s) => s.on);
  }
  export const currentSection = () => app.route.path || 'general';
</script>

<script lang="ts">
  import Icon from './Icon.svelte';

  /** `compact`: a row of chips at the top of the page (phones, where the column is a drawer). */
  let { compact = false }: { compact?: boolean } = $props();
</script>

<nav class="snav" class:compact aria-label="Settings">
  {#if !compact}
    <a class="back" href="#/" onclick={() => (app.sidebar = false)}>‹ Recipes</a>
    <h2>Settings</h2>
  {/if}
  {#each settingsSections() as s (s.id)}
    <a class="item" class:active={currentSection() === s.id} aria-current={currentSection() === s.id ? 'page' : undefined}
      href="#/settings/{s.id}" onclick={() => (app.sidebar = false)}>
      {#if !compact}<Icon name={s.icon} size={16} />{/if}<span>{s.label}</span>
    </a>
  {/each}
</nav>

<style>
  .snav { display: flex; flex-direction: column; gap: 2px; height: 100%; overflow-y: auto; padding: 12px 8px calc(8px + env(safe-area-inset-bottom)); }
  .back { color: var(--muted); text-decoration: none; font-size: 14px; padding: 4px 8px; }
  .back:hover { color: var(--text); }
  h2 { font: 700 19px var(--serif); margin: 6px 8px 10px; }
  .item { display: flex; align-items: center; gap: 8px; padding: 6px 8px; min-height: 30px; border-radius: 6px; color: var(--text); text-decoration: none; }
  .item:hover { background: var(--sunken); }
  .item.active { background: var(--accent-soft); }
  .item :global(svg) { color: var(--muted); }
  .compact { flex-direction: row; flex-wrap: wrap; gap: 6px; height: auto; padding: 0 0 4px; overflow: visible; }
  .compact .item { background: var(--sunken); border-radius: 14px; padding: 4px 12px; min-height: 32px; font-size: 14px; }
  .compact .item.active { background: var(--accent-soft); color: var(--accent-ink); }
  @media (hover: none) { .item { min-height: 40px; } }
</style>
