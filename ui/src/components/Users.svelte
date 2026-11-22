<script lang="ts">
  // Settings → People (desktop app only): everyone who signed in, their devices and what they changed.
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { toast } from '../lib/state.svelte';
  import { ask, confirmIt } from '../lib/dialog.svelte';
  import EditingAs from './EditingAs.svelte';

  interface Device { ua: string; ip: string; seen: number }
  interface Person { id: string; name: string; full_name: string; created: number; devices: Device[] }
  interface Change { t: number; method: string; path: string }

  let people = $state<Person[] | null>(null);
  let changes = $state<Record<string, Change[]>>({});
  const load = async () => { try { people = await api('GET', '/api/users'); } catch (e: any) { toast(e.message, 'error'); people = []; } };
  onMount(load);

  async function loadChanges(id: string) {
    try { changes[id] = await api('GET', `/api/users/${encodeURIComponent(id)}/activity`); } catch (e: any) { toast(e.message, 'error'); }
  }

  async function add() {
    const name = await ask('Add a person', '', 'Next', 'The name they pick when signing in on a phone or tablet.');
    if (!name?.trim()) return;
    const full_name = await ask(`${name.trim()}'s full name`, '', 'Add', 'Written on recipes they add, write or edit.');
    if (!full_name?.trim()) return;
    try { await api('POST', '/api/users', { name, full_name }); await load(); } catch (e: any) { toast(e.message, 'error'); }
  }
  async function rename(p: Person) {
    const name = await ask(`Rename ${p.name}`, p.name, 'Rename');
    if (!name?.trim() || name.trim() === p.name) return;
    try { await api('PUT', `/api/users/${encodeURIComponent(p.id)}`, { name }); await load(); } catch (e: any) { toast(e.message, 'error'); }
  }
  async function setFullName(p: Person) {
    const full_name = await ask(`${p.name}'s full name`, p.full_name, 'Save', 'Written on recipes they add, write or edit from now on.');
    if (!full_name?.trim() || full_name.trim() === p.full_name) return;
    try { await api('PUT', `/api/users/${encodeURIComponent(p.id)}`, { name: p.name, full_name }); await load(); } catch (e: any) { toast(e.message, 'error'); }
  }
  async function remove(p: Person) {
    if (!(await confirmIt(`Remove ${p.name}?`, 'Their devices are signed out. What they changed stays, and they can sign in again as someone new.', 'Remove', true))) return;
    try { await api('DELETE', `/api/users/${encodeURIComponent(p.id)}`); await load(); toast(`${p.name} removed`, 'ok'); } catch (e: any) { toast(e.message, 'error'); }
  }

  /** "Safari on iPhone" from a User-Agent (an iPad that asks for desktop sites reads as a Mac). */
  function device(ua: string) {
    const os = /iPhone/.test(ua) ? 'iPhone' : /iPad/.test(ua) ? 'iPad' : /Android/.test(ua) ? 'Android' : /Macintosh/.test(ua) ? 'Mac'
      : /Windows/.test(ua) ? 'Windows' : /CrOS/.test(ua) ? 'Chromebook' : /Linux/.test(ua) ? 'Linux' : 'Unknown device';
    const browser = /Edg\//.test(ua) ? 'Edge' : /Firefox|FxiOS/.test(ua) ? 'Firefox' : /Chrome|CriOS/.test(ua) ? 'Chrome' : /Safari/.test(ua) ? 'Safari' : '';
    return browser ? `${browser} on ${os}` : os;
  }

  const date = (t: number) => new Date(t * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
  const lastSeen = (p: Person) => Math.max(0, ...p.devices.map((d) => d.seen));

  function fileOf(path: string, prefix: string) {
    try { return decodeURIComponent(path.slice(prefix.length)); } catch { return path.slice(prefix.length); }
  }
  /** One logged request in words. */
  function describe(c: Change) {
    const { method: m, path: p } = c;
    if (p.startsWith('/api/files/')) return m === 'DELETE' ? `Moved ${fileOf(p, '/api/files/')} to the trash` : `Saved ${fileOf(p, '/api/files/')}`;
    if (p.startsWith('/api/messages')) return m === 'DELETE' ? 'Removed a whiteboard note' : 'Wrote on the whiteboard';
    if (p.startsWith('/api/import/')) return 'Imported a recipe';
    const known: Record<string, string> = {
      'POST /api/fs/move': 'Moved or renamed a file or folder', 'POST /api/fs/mkdir': 'Made a folder',
      'PUT /api/folders': 'Changed folder icons or colours', 'POST /api/trash/restore': 'Restored something from the trash',
      'DELETE /api/trash': 'Emptied the trash',
    };
    return known[`${m} ${p}`] ?? `${m} ${p}`;
  }
</script>

{#snippet history(id: string)}
  {#if !changes[id]}<p class="muted small">Loading…</p>
  {:else if !changes[id].length}<p class="muted small">No changes yet.</p>
  {:else}
    <ul class="log">
      {#each changes[id] as c}<li><span>{describe(c)}</span> <span class="muted small">{date(c.t)}</span></li>{/each}
    </ul>
  {/if}
{/snippet}

<section class="card">
  <p class="muted small">Everyone signs in with their name on each phone, tablet or other computer. Everyone can do the same things; this list only records who did what.</p>
  {#if people === null}<p class="muted">Loading…</p>
  {:else if !people.length}<p class="muted">Nobody has signed in yet.</p>
  {:else}
    <ul class="people">
      {#each people as p (p.id)}
        <li>
          <details ontoggle={(e) => e.currentTarget.open && loadChanges(p.id)}>
            <summary>
              <strong>{p.name}</strong>{#if p.full_name && p.full_name !== p.name}<span>{p.full_name}</span>{/if}
              <span class="muted small">{[`${p.devices.length} device${p.devices.length === 1 ? '' : 's'}`, lastSeen(p) && `signed in ${date(lastSeen(p))}`].filter(Boolean).join(' · ')}</span>
            </summary>
            <div class="body">
              <h3>Devices</h3>
              {#if p.devices.length}
                <ul class="log">
                  {#each p.devices as d}<li title={d.ua}><span>{device(d.ua)}</span> <span class="muted small">{d.ip} · {date(d.seen)}</span></li>{/each}
                </ul>
              {:else}<p class="muted small">Added here; not signed in anywhere yet.</p>{/if}
              <h3>Changes</h3>
              {@render history(p.id)}
              <div class="row">
                <button class="btn" onclick={() => rename(p)}>Rename…</button>
                <button class="btn" onclick={() => setFullName(p)}>{p.full_name ? 'Full name…' : 'Add full name…'}</button>
                <button class="btn danger" onclick={() => remove(p)}>Remove</button>
              </div>
            </div>
          </details>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="row"><button class="btn" onclick={add}>Add a person…</button></div>
</section>

{#if people?.length}
  <section class="card">
    <EditingAs {people} />
    <p class="muted small">Changes made in this window are credited to this person: in recipes, on the whiteboard and in the lists above.</p>
  </section>
{/if}

<section class="card">
  <details ontoggle={(e) => e.currentTarget.open && loadChanges('admin')}>
    <summary><strong>Changes made on this Mac</strong></summary>
    <div class="body">{@render history('admin')}</div>
  </details>
</section>

<style>
  /* as Settings.svelte's cards */
  section { padding: 16px 20px; display: grid; gap: 12px; }
  p { margin: 0; }
  ul { list-style: none; margin: 0; padding: 0; }
  .people > li { border-bottom: 1px solid var(--border); }
  .people > li:last-child { border-bottom: 0; }
  summary { cursor: pointer; padding: 8px 0; min-height: 36px; display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 12px; }
  .body { display: grid; gap: 8px; padding: 0 0 12px 16px; }
  h3 { margin: 4px 0 0; font: 500 14px var(--sans); color: var(--muted); }
  .log { display: grid; gap: 4px; max-height: 320px; overflow-y: auto; }
  .log li { overflow-wrap: anywhere; }
</style>
