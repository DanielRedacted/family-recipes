// Thin fetch wrapper. Every mutating call carries X-Recipes (the server's CSRF check), and every call
// names the cookbook this tab is in (`?book=`; `Bk` in crates/core/src/server.rs).

/** The cookbook the server calls the one from before cookbooks: its things in this browser keep
 * their old keys. */
const FIRST = '1';
const read = () => { try { return localStorage.getItem('cookbook') || ''; } catch { return ''; } };
/** The cookbook this tab reads and writes: this device's choice, settled by `settleBook` once the
 * session lists the cookbooks it may see. Fixed for the tab's lifetime; switching reloads the page,
 * so a page never saves into a cookbook other than the one it shows. */
export let BOOK = read();

/** Settle which cookbook this tab is in (the stored choice when it is still there, else the first).
 * Returns false when the stored choice is gone: the caller reloads into the first. */
export function settleBook(ids: string[]): boolean {
  if (BOOK && ids.includes(BOOK)) return true;
  const gone = !!BOOK;
  BOOK = ids[0] ?? '';
  try { if (BOOK) localStorage.setItem('cookbook', BOOK); } catch {}
  return !gone;
}

/** Choose another cookbook for this device and reload into it. */
export function openBook(id: string) {
  try { localStorage.setItem('cookbook', id); } catch {}
  if (read() !== id) return; // storage refused: reloading would land in the same cookbook again
  history.replaceState(null, '', '/');
  location.reload();
}

/** A key for this browser's storage that holds recipe or folder paths, one per cookbook. */
export const bookKey = (k: string) => (BOOK && BOOK !== FIRST ? `${k}@${BOOK}` : k);

/** `url` in cookbook `id` (this tab's unless given); an API address only, and only once. */
export function inBook(url: string, id = BOOK): string {
  if (!id || !url.startsWith('/api/') || /[?&]book=/.test(url)) return url;
  return url + (url.includes('?') ? '&' : '?') + 'book=' + encodeURIComponent(id);
}

export class ApiError extends Error {
  constructor(public status: number, message: string, public body: any = null) {
    super(message);
  }
}

export async function api<T = any>(method: string, path: string, body?: unknown, headers: Record<string, string> = {}): Promise<T> {
  const init: RequestInit = { method, headers: { 'X-Recipes': '1', ...headers }, credentials: 'same-origin' };
  if (body !== undefined) {
    if (body instanceof Blob || typeof body === 'string' || body instanceof ArrayBuffer || body instanceof FormData) {
      init.body = body as BodyInit;
    } else {
      init.body = JSON.stringify(body);
      (init.headers as Record<string, string>)['Content-Type'] = 'application/json';
    }
  }
  const res = await fetch(inBook(path), init);
  const text = await res.text();
  let data: any = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = text; }
  if (!res.ok) throw new ApiError(res.status, (data && data.error) || (typeof data === 'string' && data) || res.statusText, data);
  return data as T;
}

/** URL of a file in this tab's cookbook, each path segment encoded (for `<img src>` and links too). */
export function fileUrl(path: string): string {
  return inBook('/api/files/' + path.split('/').map(encodeURIComponent).join('/'));
}

/** Save a note (or any file). `etag` null = create only. Returns the new etag and final path. */
export async function saveFile(path: string, body: string | Blob, etag: string | null, unique = false) {
  const headers: Record<string, string> = etag ? { 'If-Match': `"${etag}"` } : { 'If-None-Match': '*' };
  const url = fileUrl(path);
  return api<{ path: string; etag: string }>('PUT', unique ? url + (url.includes('?') ? '&' : '?') + 'unique=true' : url, body, headers);
}
