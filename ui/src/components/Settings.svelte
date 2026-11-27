<script lang="ts">
  import { onMount } from 'svelte';
  import { app, toast, loadSession, loadVault, noteHash, dirOf, prefs, setUnits, canWrite, setDeviceAccent, groupWord, groupKindTitle } from '../lib/state.svelte';
  import { SWATCHES } from '../lib/accent';
  import type { GroupKind } from '../lib/types';
  import { api, inBook } from '../lib/api';
  import { shell, chooseCookbookFolder } from '../lib/ops';
  import { confirmIt } from '../lib/dialog.svelte';
  import { update, checkForUpdate, type UpdateMode } from '../lib/update.svelte';
  import { saveBookHtml, printBook } from '../lib/book';
  import Network from './Network.svelte';
  import GitHub from './GitHub.svelte';
  import Trash from './Trash.svelte';
  import Users from './Users.svelte';
  import Help from './Help.svelte';
  import SettingsNav, { settingsSections, currentSection } from './SettingsNav.svelte';

  // one page per section (#/settings/<section>); an unknown or hidden section shows General
  const sec = $derived(settingsSections().some((x) => x.id === currentSection()) ? currentSection() : 'general');
  const title = $derived(settingsSections().find((x) => x.id === sec)?.label ?? 'Settings');

  // light is the default everywhere; a saved "light" from before reads as that default
  let theme = $state((() => { try { return localStorage.getItem('theme') === 'dark' ? 'dark' : 'system'; } catch { return 'system'; } })());
  let s = $state<any>(null);
  let pin = $state('');
  let aiKey = $state('');
  let usdaKey = $state('');

  $effect(() => {
    try { localStorage.setItem('theme', theme); } catch {}
    if (theme === 'system') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.setAttribute('data-theme', theme);
    // the browser's own bar (theme-color, set first in theme.js) takes the page colour
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', getComputedStyle(document.documentElement).getPropertyValue('--bg').trim());
  });

  const admin = $derived(app.session.role === 'admin');

  async function refresh() { if (admin) s = await api('GET', '/api/settings'); }
  onMount(refresh);
  $effect(() => { if (admin && !s) refresh(); });

  // Settings → Calendar: the Mac's calendars, asked for once the section is open with the calendar on
  // (the first time, macOS asks whether Recipes may see the calendar)
  let cals = $state<{ access: string; calendars: { id: string; title: string }[] } | null>(null);
  $effect(() => {
    if (sec !== 'calendar' || !s?.calendar?.enabled || cals) return;
    api('GET', '/api/calendar/calendars').then((r) => (cals = r)).catch((e) => toast(e.message, 'error'));
  });
  const showCal = (id: string, on: boolean) =>
    put({ calendar: { ...s.calendar, calendars: on ? [...s.calendar.calendars, id] : s.calendar.calendars.filter((c: string) => c !== id) } });

  async function put(body: any) {
    try { s = await api('PUT', '/api/settings', body); await loadSession(); } catch (e: any) { toast(e.message, 'error'); }
  }

  const updateModes: { value: UpdateMode; label: string }[] = [
    { value: 'notify', label: 'Just tell me' },
    { value: 'download', label: 'Download it, then ask me to restart' },
    { value: 'install', label: 'Install it automatically' },
  ];
  async function setUpdates(mode: UpdateMode) {
    await put({ updates: mode });
    if (mode !== 'notify') checkForUpdate(true); // fetch one that is already out
  }

  async function savePin(clear = false) {
    if (clear && !(await confirmIt('Remove the PIN?', 'Every phone and tablet signs in again and can then only read recipes.', 'Remove PIN', true))) return;
    try {
      await api('PUT', '/api/settings/pin', { pin: clear ? '' : pin });
      pin = '';
      toast(clear ? 'PIN removed — every device signs in again' : 'PIN saved — every device signs in again with it', 'ok');
      await refresh();
      await loadSession();
    } catch (e: any) { toast(e.message, 'error'); }
  }

  // ---------- cookbooks (Settings → Cookbooks): folders the app serves; each device picks one ----------
  async function addCookbook() {
    if (await chooseCookbookFolder()) { await refresh(); toast('Cookbook added. Choose it at the top of the folder column.', 'ok'); }
  }
  async function locateCookbook(c: any) {
    if (await chooseCookbookFolder(c.id)) { await refresh(); toast(`${c.name} is back`, 'ok'); }
  }
  async function setMacOnly(c: any, on: boolean) {
    try { await api('PUT', `/api/cookbooks/${c.id}`, { mac_only: on }); c.mac_only = on; await loadSession(); }
    catch (e: any) { toast(e.message, 'error'); }
  }
  async function removeCookbook(c: any) {
    if (!(await confirmIt(`Remove ${c.name} from Recipes?`, 'Its folder and recipes stay where they are on this Mac. Devices showing it switch to another cookbook, and it stops syncing with GitHub. You can open it again later.', 'Remove', true))) return;
    try { await api('DELETE', `/api/cookbooks/${c.id}`); toast(`Removed ${c.name}`, 'ok'); await refresh(); await loadSession(); }
    catch (e: any) { toast(e.message, 'error'); }
  }

  async function saveKey() {
    try { await api('PUT', '/api/settings/ai-key', { key: aiKey }); aiKey = ''; toast('API key saved in the macOS Keychain', 'ok'); await refresh(); }
    catch (e: any) { toast(e.message, 'error'); }
  }

  async function saveUsda(clear = false) {
    try {
      await api('PUT', '/api/settings/usda-key', { key: clear ? '' : usdaKey });
      usdaKey = '';
      toast(clear ? 'USDA key removed' : 'USDA key saved in the macOS Keychain; looking up ingredients now', 'ok');
      await refresh();
    } catch (e: any) { toast(e.message, 'error'); }
  }

  // recently changed notes, leaving out files that arrived in a bulk copy (e.g. the first-run seed)
  const recent = $derived.by(() => {
    const bucket = (t: number) => Math.floor(t / 5);
    const counts = new Map<number, number>();
    for (const f of app.files) counts.set(bucket(f.mtime), (counts.get(bucket(f.mtime)) ?? 0) + 1);
    return app.files.filter((f) => f.kind === 'note' && (counts.get(bucket(f.mtime)) ?? 0) <= 20).sort((a, b) => b.mtime - a.mtime).slice(0, 10);
  });
  const ago = (t: number) => {
    const m = Math.round((Date.now() / 1000 - t) / 60);
    return m < 1 ? 'just now' : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} hr ago` : new Date(t * 1000).toLocaleDateString();
  };

  // the recipe book: the Mac saves to Downloads and says so; a browser downloads or opens its print dialog
  let saving = $state<'html' | 'pdf' | null>(null);
  async function saveBook(kind: 'html' | 'pdf') {
    saving = kind;
    try {
      const name = kind === 'html' ? await saveBookHtml() : await printBook();
      if (name) toast(`Saved “${name}” in Downloads`, 'ok');
    } catch (e: any) { toast(e.message, 'error'); }
    finally { saving = null; }
  }

  async function signOut() { await api('DELETE', '/api/session'); await loadSession(); }
</script>

{#snippet colours(label: string, current: string | null, pick: (c: string | null) => void, device: boolean)}
  {@const known = SWATCHES.some((c) => c.value === current)}
  <div class="swatches" role="group" aria-labelledby={label}>
    {#if device}
      <button type="button" class="btn same" class:on={!current} aria-pressed={!current} onclick={() => pick(null)}>Same as everyone</button>
    {/if}
    {#each SWATCHES as c, i (c.value)}
      {@const on = current === c.value || (!device && i === 0 && !current)}
      <button type="button" class="sw" class:on style="background:{c.value}" aria-pressed={on} aria-label={c.name} title={c.name}
        onclick={() => pick(!device && i === 0 ? null : c.value)}></button>
    {/each}
    <label class="sw other" class:on={!!current && !known} title="Another colour" style={current && !known ? `background:${current}` : ''}>
      <input type="color" aria-label="Another colour" value={current ?? SWATCHES[0].value} onchange={(e) => pick(e.currentTarget.value)} />
    </label>
  </div>
{/snippet}

<div class="mnav"><SettingsNav compact /></div>
{#if sec === 'trash'}
  <Trash />
{:else if sec === 'help'}
  <Help />
{:else}
<div class="page">
  <h1>{title}</h1>

  {#if sec === 'general'}
  <section class="card">
    <h2>Appearance</h2>
    <label class="row">Theme
      <select class="field w" bind:value={theme}>
        <option value="system">Light</option><option value="dark">Dark</option>
      </select>
    </label>
    <label class="row">Units
      <select class="field w" aria-label="Units" value={prefs.units} onchange={(e) => setUnits(e.currentTarget.value as 'us' | 'metric')}>
        <option value="us">US</option><option value="metric">Metric</option>
      </select>
    </label>
    <p class="muted small">US shows the recipe's own units, tidied when you scale; Metric converts to ml and g. Saved on this device.</p>
    {#if admin}
      <h3 id="colour-all">Colour for everyone</h3>
      {@render colours('colour-all', app.session.accent ?? null, (c) => put({ accent: c ?? '' }), false)}
    {/if}
    <h3 id="colour-me">{admin ? 'Colour on this Mac' : 'Colour on this device'}</h3>
    {@render colours('colour-me', prefs.accent, setDeviceAccent, true)}
    <p class="muted small">{admin ? 'Every phone and tablet uses the colour for everyone unless someone picks another one on their device.' : 'Pick a colour just for this device, or keep the one chosen on the Mac.'}</p>
  </section>

  {#if admin}
    {@const group = app.session.group ?? { kind: 'family' as GroupKind, name: '' }}
    <section class="card">
      <h2>Group</h2>
      <label class="row">Who uses Recipes
        <select class="field w" value={group.kind} onchange={(e) => put({ group: { ...group, kind: e.currentTarget.value } })}>
          <option value="family">Family</option><option value="friends">Friends</option><option value="team">Team</option><option value="other">Other</option>
        </select>
      </label>
      <label class="row">Name
        <input class="field w" maxlength="60" placeholder="Group name" value={group.name} onchange={(e) => put({ group: { ...group, name: e.currentTarget.value } })} />
      </label>
      <p class="muted small">The name shows on the sign-in screen of phones and tablets and on the People page. Without one they say “{groupKindTitle()}”.</p>
    </section>
  {/if}

  <section class="card">
    <h2>Recipe book</h2>
    <p class="muted small">Every recipe in one file: a contents page, then a chapter for each folder, in the units chosen above. The web page opens in any browser and can be emailed; the PDF is ready to print.</p>
    <div class="row">
      <button class="btn" disabled={!!saving} onclick={() => saveBook('html')}>{saving === 'html' ? 'Saving…' : 'Save as web page'}</button>
      <button class="btn" disabled={!!saving} onclick={() => saveBook('pdf')}>{saving === 'pdf' ? 'Saving…' : app.session.shell ? 'Save as PDF' : 'Print or save as PDF…'}</button>
    </div>
  </section>

  {#if !admin}
    <section class="card">
      <h2>This device</h2>
      {#if app.session.user}<p>Signed in as <strong>{app.session.user.name}</strong>.</p>{/if}
      {#if !canWrite()}<p class="muted small">Read-only: editing from phones and tablets needs a {groupWord()} PIN, set in the Recipes app on the Mac.</p>{/if}
      <div class="row"><button class="btn" onclick={signOut}>Sign out</button></div>
      {#if location.protocol === 'http:'}
        <p class="muted small">To add Recipes to the home screen as an app that keeps working while the Mac is asleep, do the one-time setup.</p>
        <a class="btn" href="/setup">Install as an app</a>
      {/if}
    </section>
  {/if}
  <p class="muted small">Recipes {app.session.version} · {app.files.filter((f) => f.kind === 'note').length} notes</p>
  {:else if sec === 'recent'}
  <section class="card">
    {#if recent.length}
      <ul class="recent">
        {#each recent as f (f.path)}<li><a href={noteHash(f.path)}>{f.title}</a> <span class="muted small">{dirOf(f.path)} · {ago(f.mtime)}</span></li>{/each}
      </ul>
    {:else}<p class="muted small">Nothing has been changed since the recipes were added.</p>{/if}
  </section>
  {:else if sec === 'people'}
    <Users />
  {:else if !s}
    <p class="muted">Loading…</p>
  {:else if sec === 'folder'}
    {@const many = s.cookbooks.length > 1}
    <p class="muted small">Every recipe is a plain text (Markdown) file in a cookbook's folder. You can also open and edit them with any text editor or Obsidian; changes show up here instantly.</p>
    {#each s.cookbooks as c (c.id)}
      <section class="card">
        <h2>{c.name}</h2>
        <p class="mono">{c.path}</p>
        {#if c.missing}
          <p class="err small">This folder isn't there. If it moved, choose where it is now; if it's on a drive, connect the drive and open Recipes again.</p>
        {/if}
        <div class="row">
          {#if app.session.shell && !c.missing}<button class="btn" onclick={() => api('POST', inBook('/api/shell/reveal', c.id), { path: '' }).catch((e) => toast(e.message, 'error'))}>Show in Finder</button>{/if}
          {#if app.session.shell && c.missing}<button class="btn" onclick={() => locateCookbook(c)}>Choose its folder…</button>{/if}
          {#if many}<button class="btn danger" onclick={() => removeCookbook(c)}>Remove</button>{/if}
        </div>
        {#if many}
          <label class="row toggle"><input type="checkbox" checked={c.mac_only} onchange={(e) => setMacOnly(c, e.currentTarget.checked)} /> Only on this Mac</label>
          <p class="muted small">Phones, tablets and other computers don't see it, or its meals on the Calendar.</p>
        {/if}
        {#if !c.missing}
          <h3>GitHub</h3>
          <GitHub book={c.id} status={c.git} onchange={(g) => (c.git = g)} />
        {/if}
      </section>
    {/each}
    {#if app.session.shell}
      <section class="card">
        <h2>Another cookbook</h2>
        <p class="muted small">Keep a second collection apart from this one, in its own folder, with its own GitHub repository if you like. Choose a folder of recipes, or make a new folder in the window that opens. Each device then picks its cookbook at the top of the folder column; people, the whiteboard and the meal history are shared.</p>
        <div class="row"><button class="btn" onclick={addCookbook}>Open or create a cookbook…</button></div>
      </section>
    {/if}
  {:else if sec === 'app'}
      <section class="card">
        <label class="row toggle"><input type="checkbox" checked={!!s.autostart} onchange={async (e) => { const r = await shell('autostart', { enabled: e.currentTarget.checked }); if (r) s.autostart = r.enabled; }} /> Open Recipes at login</label>
        <p class="muted small">Closing the window keeps Recipes running in the menu bar, so phones and tablets can still reach it. Quit from the menu-bar icon.</p>
      </section>
      <section class="card">
        <h2>Updates</h2>
        <p class="muted small">Recipes {app.session.version}. New versions are looked for each time the app opens{s.updates === 'notify' ? '' : ', and every 6 hours while it runs'}.</p>
        <fieldset class="cals">
          <legend class="small">When there is a new version</legend>
          {#each updateModes as m (m.value)}
            <label class="row"><input type="radio" name="updates" value={m.value} checked={s.updates === m.value} onchange={() => setUpdates(m.value)} /> {m.label}</label>
          {/each}
        </fieldset>
        {#if s.updates === 'install'}<p class="muted small">Recipes restarts into the new version when its window is closed and nothing is unsaved, or when you choose Restart to update. Phones and tablets lose the connection for a few seconds.</p>{/if}
        <div class="row"><button class="btn" onclick={() => checkForUpdate()} disabled={update.checking}>{update.checking ? 'Checking…' : 'Check for updates'}</button></div>
      </section>
  {:else if sec === 'devices'}
    <section class="card">
      <p class="muted small">Phones, tablets and other computers on the same Wi-Fi can open the recipes.</p>
      <Network status={s.network} lan={s.lan} keepAwake={s.keep_awake} onchange={put} />
      <h3>{groupKindTitle()} PIN</h3>
      <p class="muted small">Asked once on each phone, tablet or other computer when someone signs in with their name. With it, everyone signed in can edit, import and delete. {s.pin_set ? 'A PIN is set.' : 'No PIN is set, so other devices can read recipes and use the whiteboard, but not edit.'}</p>
      <form class="row" onsubmit={(e) => { e.preventDefault(); savePin(); }}>
        <input class="field w" type="password" placeholder={s.pin_set ? 'New PIN' : 'Choose a PIN'} bind:value={pin} autocomplete="new-password" aria-label="PIN" />
        <button class="btn primary" disabled={!pin.trim()}>{s.pin_set ? 'Change PIN' : 'Set PIN'}</button>
        {#if s.pin_set}<button type="button" class="btn danger" onclick={() => savePin(true)}>Remove PIN</button>{/if}
      </form>
    </section>
  {:else if sec === 'calendar'}
    <section class="card">
      <label class="row toggle"><input type="checkbox" checked={s.calendar.enabled} onchange={(e) => put({ calendar: { ...s.calendar, enabled: e.currentTarget.checked } })} /> Show events from this Mac’s calendar</label>
      <p class="muted small">Off by default. When on, today’s events appear on the home page and each day’s events on the Calendar page, for everyone signed in on this Mac, phones and tablets. Recipes only reads the calendar: add or change events in the Calendar app, and Recipes shows the change within a minute.</p>
      {#if s.calendar.enabled}
        {#if !cals}
          <p class="muted small">Looking for calendars…</p>
        {:else if cals.access !== 'ok'}
          <p class="small">Recipes isn’t allowed to see the calendar. Turn it on in System Settings → Privacy &amp; Security → Calendars, then open this page again.</p>
        {:else if !cals.calendars.length}
          <p class="muted small">This Mac has no calendars. Add one in the Calendar app.</p>
        {:else}
          <fieldset class="cals">
            <legend class="small">Calendars to show</legend>
            {#each cals.calendars as c (c.id)}
              <label class="row"><input type="checkbox" checked={s.calendar.calendars.includes(c.id)} onchange={(e) => showCal(c.id, e.currentTarget.checked)} /> {c.title}</label>
            {/each}
          </fieldset>
          <p class="muted small">{s.calendar.calendars.length ? 'Only the ticked calendars are shown.' : 'Nothing is shown until you tick a calendar.'} macOS lets Recipes see every calendar on this Mac, but Recipes reads only the ones ticked here.</p>
        {/if}
      {/if}
    </section>
  {:else if sec === 'nutrition'}
    <section class="card">
      <p class="muted small">Recipes show a Nutrition Facts label worked out from their ingredients, using values from USDA FoodData Central that come with the app. With a free USDA API key, the app also looks up ingredients that aren't in its list yet. Only ingredient names are sent to USDA. Check or change what it found on the Ingredients page.</p>
      <form class="keyrow" onsubmit={(e) => { e.preventDefault(); saveUsda(); }}>
        <label>USDA API key {s.usda_key_set ? '(saved)' : ''}
          <input class="field" type="password" bind:value={usdaKey} placeholder={s.usda_key_set ? '••••••••  (enter a new key to replace)' : 'Paste your API key'} autocomplete="off" />
        </label>
        <button class="btn" disabled={!usdaKey.trim()}>Save key</button>
        {#if s.usda_key_set}<button type="button" class="btn ghost" onclick={() => saveUsda(true)}>Remove key</button>{/if}
      </form>
      <p class="muted small">Get a free key at <a href="https://fdc.nal.usda.gov/api-key-signup">fdc.nal.usda.gov/api-key-signup</a>. It's kept in the macOS Keychain.</p>
      {#if s.nutrition_status}<p class="small">{s.nutrition_status}</p>{/if}
    </section>
  {:else if sec === 'ai'}
    <section class="card">
      <label class="row toggle"><input type="checkbox" checked={s.ai.enabled} onchange={(e) => put({ ai: { ...s.ai, enabled: e.currentTarget.checked } })} /> Enable AI features</label>
      <p class="muted small">Off by default. When on, AI can help fix imported recipes and answer questions about a recipe — always as a suggestion you accept or reject. Uses your own API key, or a model running on this Mac (Ollama, LM Studio): choose OpenAI-compatible and enter its address, e.g. <code>http://localhost:11434/v1</code>; no key needed.</p>
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
              <input class="field" value={s.ai.base_url} placeholder="https://api.openai.com/v1 or http://localhost:11434/v1" onchange={(e) => put({ ai: { ...s.ai, base_url: e.currentTarget.value } })} />
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
  {/if}
</div>
{/if}

<style>
  .page { padding: 16px 28px 80px; max-width: 760px; display: grid; gap: 16px; }
  h1 { margin: 8px 0 0; }
  section { padding: 16px 20px; display: grid; gap: 12px; }
  section h2 { margin: 0; font-size: 1.15rem; }
  section h3 { margin: 12px 0 0; font-size: 1rem; }
  p { margin: 0; }
  .w { width: auto; min-width: 220px; }
  .mono { font-family: var(--mono); font-size: 13px; overflow-wrap: anywhere; }
  .toggle { gap: 8px; font-weight: 500; }
  .grid2 { display: grid; gap: 12px; }
  .grid2 label { display: grid; gap: 4px; font-size: 14px; }
  .keyrow { display: flex; gap: 8px; align-items: end; }
  .cals { border: 0; margin: 0; padding: 0; display: grid; gap: 4px; }
  .cals legend { padding: 0; margin-bottom: 4px; font-weight: 500; }
  .cals label { gap: 8px; min-height: 36px; flex-wrap: nowrap; }
  .keyrow label { flex: 1; }
  .recent { margin: 0; padding-left: 20px; display: grid; gap: 4px; }
  .swatches { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
  .swatches .same { min-height: 32px; }
  .swatches .same.on { border-color: var(--accent-ink); box-shadow: 0 0 0 2px var(--accent-soft); }
  /* the native colour well, hidden inside a rainbow swatch until a colour of its own is chosen */
  .sw.other { position: relative; overflow: hidden; background: conic-gradient(#c23a4b, #c25a1c, #d9b310, #3d8a3a, #14837b, #2f6fd0, #7a52cc, #c2408f, #c23a4b); }
  .sw.other input { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer; border: 0; padding: 0; }
  .sw.other:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
  .mnav { display: none; }
  @media (max-width: 899px) { .page { padding: 8px 16px 96px; } .mnav { display: block; padding: 12px 16px 0; } }
</style>
