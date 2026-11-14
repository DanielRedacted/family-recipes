import { api } from './api';
import type { Entry, Session } from './types';
import { buildIndex } from './search';

export const app = $state({
  session: { role: 'reader', pin_set: false, ai: false, shell: false, version: '' } as Session,
  files: [] as Entry[],
  byPath: {} as Record<string, Entry>,
  folders: [] as string[],
  loaded: false,
  offline: false,
  route: parseRoute(),
  sidebar: false, // mobile drawer open
  toasts: [] as { id: number; text: string; kind: string }[],
  pinPrompt: false,
  switcher: false,
});

export const canWrite = () => app.session.role !== 'reader';

export function toast(text: string, kind: 'info' | 'error' | 'ok' = 'info') {
  const id = Math.random();
  app.toasts.push({ id, text, kind });
  setTimeout(() => { app.toasts = app.toasts.filter((t) => t.id !== id); }, kind === 'error' ? 6000 : 3000);
}

// ---------- routing: #/n/<path> note, #/f/<folder> folder, #/settings, #/trash, #/import ----------

export interface Route { view: string; path: string; mode?: string }

export function parseRoute(): Route {
  const h = decodeURIComponent(location.hash.replace(/^#\/?/, ''));
  const [view, ...rest] = h.split('/');
  const path = rest.join('/');
  if (view === 'n') {
    const [p, mode] = path.split('?');
    return { view: 'note', path: p, mode: new URLSearchParams(mode || '').get('mode') || undefined };
  }
  if (view === 'f') return { view: 'folder', path };
  if (['settings', 'trash', 'import', 'search'].includes(view)) return { view, path };
  return { view: 'folder', path: '' };
}

export function go(hash: string) {
  if (location.hash !== hash) location.hash = hash;
  else app.route = parseRoute();
  app.sidebar = false;
}

export const noteHash = (p: string, mode?: string) => '#/n/' + p.split('/').map(encodeURIComponent).join('/') + (mode ? `?mode=${mode}` : '');
export const folderHash = (p: string) => '#/f/' + p.split('/').map(encodeURIComponent).join('/');

window.addEventListener('hashchange', () => { app.route = parseRoute(); });

// ---------- data ----------

export async function loadSession() {
  try { app.session = await api<Session>('GET', '/api/session'); } catch { /* offline */ }
}

let loading: Promise<void> | null = null;
export function loadVault(): Promise<void> {
  loading ??= (async () => {
    try {
      const v = await api<{ files: Entry[]; folders: string[] }>('GET', '/api/vault');
      app.files = v.files;
      app.byPath = Object.fromEntries(v.files.map((f) => [f.path, f]));
      app.folders = v.folders;
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

/** Resolve a [[wiki link]] by filename stem, then by title. */
export function resolveLink(target: string): Entry | undefined {
  const t = target.trim().toLowerCase();
  return app.files.find((f) => f.kind === 'note' && stemOf(f.path).toLowerCase() === t)
    ?? app.files.find((f) => f.title.toLowerCase() === t)
    ?? app.files.find((f) => f.path.toLowerCase() === t || f.path.toLowerCase() === t + '.md');
}
