// Thin fetch wrapper. Every mutating call carries X-Recipes (the server's CSRF check).

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
  const res = await fetch(path, init);
  const text = await res.text();
  let data: any = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = text; }
  if (!res.ok) throw new ApiError(res.status, (data && data.error) || (typeof data === 'string' && data) || res.statusText, data);
  return data as T;
}

/** URL of a vault file, each path segment encoded. */
export function fileUrl(path: string): string {
  return '/api/files/' + path.split('/').map(encodeURIComponent).join('/');
}

export function downloadUrl(path = ''): string {
  return '/api/download' + (path ? '?path=' + encodeURIComponent(path) : '');
}

/** Save a note (or any file). `etag` null = create only. Returns the new etag and final path. */
export async function saveFile(path: string, body: string | Blob, etag: string | null, unique = false) {
  const headers: Record<string, string> = etag ? { 'If-Match': `"${etag}"` } : { 'If-None-Match': '*' };
  return api<{ path: string; etag: string }>('PUT', fileUrl(path) + (unique ? '?unique=1' : ''), body, headers);
}
