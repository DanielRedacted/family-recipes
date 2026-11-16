import { api } from './api';
import type { Entry, Session } from './types';
import { buildIndex, aliasesOf } from './search';
import { parse as parseYaml } from 'yaml';

export interface FolderMeta { emoji?: string; color?: string }

export const app = $state({
  session: { role: 'reader', pin_set: false, ai: false, shell: false, version: '' } as Session,
  files: [] as Entry[],
  byPath: {} as Record<string, Entry>,
  folders: [] as string[],
  /** .folders.yaml as written (edited with yaml's Document API to keep comments) and parsed */
  folderMetaRaw: '',
  folderMeta: {} as Record<string, FolderMeta>,
  loaded: false,
  offline: false,
  route: { view: 'folder', path: '' } as Route,
  sidebar: false, // mobile drawer open
  toasts: [] as { id: number; text: string; kind: string }[],
  pinPrompt: false,
  switcher: false,
  /** open tabs (the strip above the page column) and the active one's id */
  tabs: [] as Tab[],
  tab: 0,
  /** raised on every page change in a tab (not on switching tabs) */
  nav: 0,
});

export const canWrite = () => app.session.role !== 'reader';

/** Reading preferences for this device (Settings → General), kept in this browser. */
export const prefs = $state({
  units: ((): 'us' | 'metric' => { try { return localStorage.getItem('units') === 'metric' ? 'metric' : 'us'; } catch { return 'us'; } })(),
});
export function setUnits(u: 'us' | 'metric') {
  prefs.units = u;
  try { localStorage.setItem('units', u); } catch {}
}

export function toast(text: string, kind: 'info' | 'error' | 'ok' = 'info') {
  const id = Math.random();
  app.toasts.push({ id, text, kind });
  setTimeout(() => { app.toasts = app.toasts.filter((t) => t.id !== id); }, kind === 'error' ? 6000 : 3000);
}

// ---------- routing: #/n/<path> note, #/f/<folder> folder, #/settings, #/trash, #/import ----------

export interface Route { view: string; path: string; mode?: string; /** the hash this route was read from */ hash?: string }

export function parseRoute(hash = location.hash): Route {
  const h = decodeURIComponent(hash.replace(/^#\/?/, ''));
  const [view, ...rest] = h.split('/');
  const path = rest.join('/');
  if (view === 'n') {
    const [p, mode] = path.split('?');
    return { view: 'note', path: p, mode: new URLSearchParams(mode || '').get('mode') || undefined, hash };
  }
  if (view === 'f') return { view: 'folder', path, hash };
  if (view === 'trash') return { view: 'settings', path: 'trash', hash };
  if (['settings', 'import', 'search', 'ingredients'].includes(view)) return { view, path, hash };
  return { view: 'folder', path: '', hash };
}

export const noteHash = (p: string, mode?: string) => '#/n/' + p.split('/').map(encodeURIComponent).join('/') + (mode ? `?mode=${mode}` : '');
export const folderHash = (p: string) => '#/f/' + p.split('/').map(encodeURIComponent).join('/');
/** the hash as the browser writes it (location.hash), so hashes compare equal */
const norm = (hash: string) => new URL(hash || '#/', location.href).hash || '#/';
/** tabs need room: two columns (phones show one page, without the tab strip) */
export const wide = () => typeof matchMedia !== 'undefined' && matchMedia('(min-width: 900px)').matches;

// ---------- tabs ----------
// Each tab keeps its own page alive and its own list of pages for Back. The browser's history holds
// every step too (tagged with its tab), so a browser's Back and a phone's swipe keep working; entries
// are numbered 0, 1, 2… in order, so the entry before the current one is always `navIndex - 1`.

export interface Tab { id: number; route: Route; stack: { hash: string; nav: number }[]; pos: number }

/** the active tab (`app.route` is its route) */
export const activeTab = () => app.tabs.find((t) => t.id === app.tab) ?? app.tabs[0];

// A page with unsaved work can ask before its tab's page changes or the tab closes (resolve true to leave).
type Guard = () => Promise<boolean>;
const guards = new Map<number, Guard>();
export const setLeaveGuard = (tab: number, g: Guard) => { guards.set(tab, g); };
export const clearLeaveGuard = (tab: number, g: Guard) => { if (guards.get(tab) === g) guards.delete(tab); };
/** Ask every open page about unsaved work, as closing them would (true = fine to go). */
export async function mayLeave() {
  for (const g of [...guards.values()]) if (!(await g())) return false;
  return true;
}

let currentHash = '';
let navIndex = 0;
let busy = false;

function saveTabs() {
  try { localStorage.setItem('tabs', JSON.stringify({ active: app.tab, tabs: app.tabs.map(({ id, stack, pos }) => ({ id, stack, pos })) })); } catch {}
}

/** Browser entries from `n` on were just replaced by a new one: steps that pointed at them don't any more. */
function drop(n: number) {
  for (const t of app.tabs) for (const s of t.stack) if (s.nav >= n) s.nav = -1;
}

/** Show `t` at its current step. */
function show(t: Tab) {
  const hash = t.stack[t.pos].hash;
  if (t.route.hash !== hash) { t.route = parseRoute(hash); app.nav++; }
  app.tab = t.id;
  app.route = t.route;
  currentHash = hash;
  saveTabs();
}

/** A new page in `t` (after its current one; the steps it had gone back over are gone). */
function push(t: Tab, hash: string, nav: number) {
  t.stack = [...t.stack.slice(Math.max(0, t.pos - 48), t.pos + 1), { hash, nav }];
  t.pos = t.stack.length - 1;
}

export function go(hash: string) {
  hash = norm(hash);
  if (location.hash !== hash) location.hash = hash;
  else { const t = activeTab(); t.route = parseRoute(hash); app.route = t.route; app.nav++; } // the same page again (Home tapped)
  app.sidebar = false;
}

/** The browser moved: a new page (a link, `go`) or its Back / Forward. */
async function arrive() {
  const next = location.hash || '#/';
  const st = history.state;
  if (busy) return;
  if (typeof st?.nav === 'number' ? st.nav === navIndex && st.tab === app.tab : next === currentHash) return;
  if (typeof st?.nav !== 'number') {
    const t = activeTab();
    const g = guards.get(t.id);
    if (g) {
      history.replaceState({ nav: navIndex, tab: t.id }, '', currentHash); // stay put while asking
      busy = true;
      const ok = await g().finally(() => (busy = false));
      if (!ok) return;
      guards.delete(t.id);
      history.pushState({ nav: ++navIndex, tab: t.id }, '', next);
    } else history.replaceState({ nav: ++navIndex, tab: t.id }, '');
    drop(navIndex);
    push(t, next, navIndex);
    return show(t);
  }
  // Back or Forward: to that entry's tab (the current one when it was closed) and step
  const t = app.tabs.find((x) => x.id === st.tab) ?? activeTab();
  const g = guards.get(t.id);
  if (g && next !== t.stack[t.pos].hash) {
    const a = activeTab();
    history.replaceState({ nav: st.nav, tab: a.id }, '', currentHash);
    busy = true;
    const ok = await g().finally(() => (busy = false));
    if (!ok) { // stayed: the current page now lives in this entry
      for (const x of app.tabs) for (const s of x.stack) if (s.nav === st.nav) s.nav = -1;
      a.stack[a.pos].nav = navIndex = st.nav;
      return saveTabs();
    }
    guards.delete(t.id);
    history.replaceState(st, '', next);
  }
  navIndex = st.nav;
  const i = t.stack.findIndex((s) => s.nav === st.nav);
  if (i >= 0) t.pos = i;
  else push(t, next, st.nav);
  show(t);
}
window.addEventListener('hashchange', arrive);
// two tabs on the same page differ only in their entry's state, which fires no hashchange
window.addEventListener('popstate', arrive);

/** Back to the active tab's previous page (the Back button, ⌘[). */
export async function back() {
  const t = activeTab();
  if (t.pos === 0) return;
  const prev = t.stack[t.pos - 1];
  if (prev.nav >= 0 && prev.nav === navIndex - 1) return history.back(); // the browser's own Back, as a swipe would
  const g = guards.get(t.id);
  if (g) { if (!(await g())) return; guards.delete(t.id); }
  history.pushState({ nav: ++navIndex, tab: t.id }, '', prev.hash);
  drop(navIndex);
  prev.nav = navIndex;
  t.pos--;
  show(t);
}

export function selectTab(id: number) {
  const t = app.tabs.find((x) => x.id === id);
  if (!t || id === app.tab) return;
  history.pushState({ nav: ++navIndex, tab: id }, '', t.stack[t.pos].hash);
  drop(navIndex);
  t.stack[t.pos].nav = navIndex;
  show(t);
}

/** Open `hash` in a new tab, the right-most one. */
export function newTab(hash = '#/') {
  const id = Math.max(0, ...app.tabs.map((t) => t.id)) + 1;
  app.tabs.push({ id, route: { view: '', path: '' }, stack: [{ hash: norm(hash), nav: -1 }], pos: 0 });
  selectTab(id);
  app.sidebar = false;
}

export async function closeTab(id: number) {
  const i = app.tabs.findIndex((t) => t.id === id);
  if (i < 0) return;
  const g = guards.get(id);
  if (g && !(await g())) return;
  guards.delete(id);
  app.tabs.splice(i, 1);
  if (!app.tabs.length) return newTab();
  if (id === app.tab) selectTab(app.tabs[Math.min(i, app.tabs.length - 1)].id);
  else saveTabs();
}

export async function closeOtherTabs(id: number) {
  for (const t of app.tabs.filter((t) => t.id !== id)) await closeTab(t.id);
}

export function moveTab(id: number, to: number) {
  const i = app.tabs.findIndex((t) => t.id === id);
  if (i < 0 || i === to) return;
  const [t] = app.tabs.splice(i, 1);
  app.tabs.splice(to, 0, t);
  saveTabs();
}

/** `from` (a file or folder) moved to `to`, or went to the trash (null): every tab showing it follows. */
export function retarget(from: string, to: string | null) {
  const map = (hash: string) => {
    const r = parseRoute(hash);
    if ((r.view !== 'note' && r.view !== 'folder') || (r.path !== from && !r.path.startsWith(from + '/'))) return hash;
    if (to === null) return norm(folderHash(dirOf(from)));
    const np = to + r.path.slice(from.length);
    return norm(r.view === 'folder' ? folderHash(np) : noteHash(np));
  };
  const a = activeTab();
  const cur = a.stack[a.pos];
  for (const t of app.tabs) for (const s of t.stack) if (s !== cur) s.hash = map(s.hash);
  for (const t of app.tabs) if (t !== a && t.route.hash !== t.stack[t.pos].hash) t.route = parseRoute(t.stack[t.pos].hash);
  saveTabs();
  const next = map(cur.hash);
  if (next === cur.hash) return;
  if (guards.has(a.id)) return go(next); // asks about unsaved changes first
  cur.hash = next;
  history.replaceState(history.state, '', next);
  show(a);
}

// start: the tabs this device had open; a new window (no history yet) keeps their pages, not their Back steps
{
  const fresh = typeof history.state?.nav !== 'number';
  navIndex = fresh ? 0 : history.state.nav;
  let saved: { active?: number; tabs?: { id: number; stack: { hash: string; nav: number }[]; pos: number }[] } = {};
  try { saved = JSON.parse(localStorage.getItem('tabs') || '{}') ?? {}; } catch {}
  const tabs = (Array.isArray(saved.tabs) ? saved.tabs : [])
    .filter((t) => typeof t?.id === 'number' && Array.isArray(t.stack) && t.stack.length && t.stack.every((s) => typeof s?.hash === 'string'))
    .map((t) => {
      const pos = Math.min(Math.max(0, t.pos | 0), t.stack.length - 1);
      const stack = fresh ? [{ hash: t.stack[pos].hash, nav: -1 }] : t.stack.map((s) => ({ hash: s.hash, nav: typeof s.nav === 'number' ? s.nav : -1 }));
      return { id: t.id, stack, pos: fresh ? 0 : pos, route: parseRoute(stack[fresh ? 0 : pos].hash) };
    });
  app.tabs = tabs;
  let t = app.tabs.find((x) => x.id === (history.state?.tab ?? saved.active)) ?? app.tabs[0];
  const here = location.hash && location.hash !== '#' ? location.hash : '';
  if (!t) {
    app.tabs.push({ id: 1, route: { view: '', path: '' }, stack: [{ hash: here || '#/', nav: navIndex }], pos: 0 });
    t = app.tabs[0];
  } else if (here && here !== t.stack[t.pos].hash) {
    // a link opened in this app: in the tab, as a new page (a new window: instead of its page)
    if (fresh) t.stack[t.pos] = { hash: here, nav: navIndex };
    else push(t, here, navIndex);
  }
  for (const x of app.tabs) for (const s of x.stack) if (s.nav === navIndex) s.nav = -1;
  t.stack[t.pos].nav = navIndex;
  history.replaceState({ ...(history.state ?? {}), nav: navIndex, tab: t.id }, '', t.stack[t.pos].hash);
  show(t);
}

// ---------- data ----------

export async function loadSession() {
  try { app.session = await api<Session>('GET', '/api/session'); } catch { /* offline */ }
}

let loading: Promise<void> | null = null;
export function loadVault(): Promise<void> {
  loading ??= (async () => {
    try {
      const v = await api<{ files: Entry[]; folders: string[]; folder_meta?: string }>('GET', '/api/vault');
      app.files = v.files;
      app.byPath = Object.fromEntries(v.files.map((f) => [f.path, f]));
      app.folders = v.folders;
      app.folderMetaRaw = v.folder_meta ?? '';
      try { const m = parseYaml(app.folderMetaRaw || '{}'); app.folderMeta = m && typeof m === 'object' ? m : {}; } catch { app.folderMeta = {}; }
      app.offline = false;
      buildIndex(v.files);
    } catch (e) {
      app.offline = true;
    } finally {
      app.loaded = true;
      loading = null;
    }
  })();
  return loading;
}

export function listenForChanges() {
  let es: EventSource | null = null;
  const connect = () => {
    es = new EventSource('/api/events');
    es.addEventListener('vault', () => loadVault());
    es.onerror = () => { es?.close(); setTimeout(connect, 5000); };
    es.onopen = () => loadVault();
  };
  connect();
}

/** Folder of a vault path ('' for root). */
export const dirOf = (p: string) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/')) : '');
export const nameOf = (p: string) => p.slice(p.lastIndexOf('/') + 1);
export const stemOf = (p: string) => nameOf(p).replace(/\.[^.]+$/, '');

/** Resolve a [[wiki link]] by filename stem, then title, then `aliases`. */
export function resolveLink(target: string): Entry | undefined {
  const t = target.trim().toLowerCase();
  return app.files.find((f) => f.kind === 'note' && stemOf(f.path).toLowerCase() === t)
    ?? app.files.find((f) => f.title.toLowerCase() === t)
    ?? app.files.find((f) => f.kind === 'note' && aliasesOf(f).some((a) => a.toLowerCase() === t))
    ?? app.files.find((f) => f.path.toLowerCase() === t || f.path.toLowerCase() === t + '.md');
}
