import { api } from './api';
import { normalizeUnits } from './units';

export interface ImageCand { i: number; alt: string; kind: string; step?: string | null; url: string }
export interface Issue { level: 'warn' | 'info'; message: string }
export interface LogStep { ms: number; step: string; detail: string; ok: boolean }
export interface ImportJob {
  job: string;
  title: string;
  markdown: string;
  source_text: string;
  method: string;
  images: ImageCand[];
  issues: Issue[];
  duplicate: string | null;
  original: string | null;
}
/** The user's edits on the review screen; kept on the item so switching items loses nothing. */
export interface Review { md: string; name: string; chosen: Record<number, boolean>; cover: number | null; hidden: Record<number, boolean>; issues: Issue[] }

export type Status = 'choose' | 'waiting' | 'running' | 'ready' | 'failed';
export interface QueueItem {
  id: number;
  label: string;
  kind: 'url' | 'file' | 'vault';
  status: Status;
  url?: string;
  file?: File;
  jobId?: string;
  log: LogStep[];
  error?: string;
  job?: ImportJob;
  review?: Review;
  /** a file already in the recipes folder: converted where it is (it moves under attachments/originals on save) */
  vaultPath?: string;
  /** PDFs and photos: keep as view-only (title/tags) or convert */
  keepTitle?: string;
  keepTags?: string;
}

/**
 * Import queue. Usually one item; several when the user drops many files or pastes many
 * addresses. Items are processed one at a time; each can be reviewed as soon as it is ready.
 */
export const importer = $state({
  queue: [] as QueueItem[],
  selected: null as number | null,
  /** set by the "+" menu: which input the import page should open */
  focus: null as null | 'url' | 'file' | 'text',
  /** destination folder preset by the "+" menu */
  folder: null as string | null,
});

let nextId = 1;
const get = (id: number) => importer.queue.find((q) => q.id === id);
const isKeepable = (f: File) => /\.pdf$/i.test(f.name) || f.type.startsWith('image/');
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** DOCX → HTML in the browser (mammoth keeps lists, tables, bold, headings, embedded images). */
export async function docxToHtml(file: File): Promise<string> {
  const mammoth: any = await import('mammoth');
  const r = await (mammoth.default ?? mammoth).convertToHtml({ arrayBuffer: await file.arrayBuffer() });
  return r.value as string;
}

export async function importFile(file: File, mode: 'keep' | 'ingest', extra: Record<string, string> = {}): Promise<any> {
  const fd = new FormData();
  fd.append('mode', mode);
  for (const [k, v] of Object.entries(extra)) fd.append(k, v);
  if (mode === 'ingest' && /\.docx$/i.test(file.name)) fd.append('html', await docxToHtml(file));
  fd.append('file', file, file.name);
  return api('POST', '/api/import/file', fd);
}

/** Recipe addresses from pasted text (one or many, any separators). */
export function enqueueUrls(text: string): number {
  const urls = [...new Set(text.split(/\s+/).map((u) => u.trim()).filter((u) => /^https?:\/\/\S+\.\S+/i.test(u)))]
    .filter((u) => !importer.queue.some((q) => q.url === u));
  for (const url of urls) importer.queue.push({ id: nextId++, label: url, kind: 'url', status: 'waiting', url, log: [] });
  pump();
  return urls.length;
}

export function enqueueFiles(files: FileList | File[], vaultPath?: string): number {
  const list = [...files];
  for (const file of list) {
    importer.queue.push({
      id: nextId++, label: file.name, kind: 'file', file, log: [], vaultPath,
      status: isKeepable(file) && !vaultPath ? 'choose' : 'waiting',
      keepTitle: file.name.replace(/\.[^.]+$/, ''), keepTags: '',
    });
  }
  pump();
  return list.length;
}

export function enqueueText(text: string) {
  enqueueFiles([new File([text], 'Pasted text.txt', { type: 'text/plain' })]);
}

/** A job the server already started (e.g. "Convert this PDF" in the viewer). */
export function enqueueStarted(label: string, jobId: string) {
  const id = nextId++;
  importer.queue.push({ id, label, kind: 'vault', status: 'running', jobId, log: [] });
  importer.selected = id;
  poll(id);
}

export function convert(id: number) {
  const it = get(id);
  if (it) { it.status = 'waiting'; pump(); }
}

/** Keep a PDF/photo as a view-only recipe. Returns the new note's path. */
export async function keep(id: number, folder: string): Promise<string> {
  const it = get(id);
  if (!it?.file) throw new Error('nothing to keep');
  const r = await importFile(it.file, 'keep', { title: it.keepTitle ?? '', folder, tags: it.keepTags ?? '' });
  remove(id, false);
  return r.path;
}

export function retry(id: number) {
  const it = get(id);
  if (!it) return;
  if (it.kind === 'vault') { remove(id); return; }
  Object.assign(it, { status: 'waiting', error: undefined, log: [], jobId: undefined, job: undefined, review: undefined });
  pump();
}

/** Drop an item (and the server's draft unless it was just saved). */
export function remove(id: number, cancelJob = true) {
  const i = importer.queue.findIndex((q) => q.id === id);
  if (i < 0) return;
  const it = importer.queue[i];
  if (cancelJob && it.jobId) api('DELETE', `/api/import/${it.jobId}`).catch(() => {});
  importer.queue.splice(i, 1);
  if (importer.selected === id) importer.selected = importer.queue.find((q) => q.status === 'ready')?.id ?? null;
}

function pump() {
  if (importer.queue.some((q) => q.status === 'running' && q.kind !== 'vault')) return;
  const next = importer.queue.find((q) => q.status === 'waiting');
  if (next) run(next.id);
}

async function run(id: number) {
  const it = get(id);
  if (!it) return;
  it.status = 'running';
  it.log = [];
  try {
    const r = it.kind === 'url' ? await api('POST', '/api/import/url', { url: it.url }) : await importFile(it.file!, 'ingest', it.vaultPath ? { vault_path: it.vaultPath } : {});
    const cur = get(id);
    if (!cur) return;
    cur.jobId = r.job;
    await poll(id);
  } catch (e: any) {
    const cur = get(id);
    if (cur) { cur.status = 'failed'; cur.error = e.message; }
  } finally {
    pump();
  }
}

async function poll(id: number) {
  for (;;) {
    const it = get(id);
    if (!it?.jobId) return;
    let s: any;
    try { s = await api('GET', `/api/import/${it.jobId}`); }
    catch (e: any) { it.status = 'failed'; it.error = e.message; return; }
    const cur = get(id);
    if (!cur) return;
    cur.log = s.log ?? [];
    if (s.status === 'running') { await sleep(400); continue; }
    if (s.status === 'failed') { cur.status = 'failed'; cur.error = s.error; return; }
    const job = s as ImportJob;
    const first = job.images.find((c) => c.kind === 'recipe' || c.kind === 'document') ?? null;
    cur.job = job;
    cur.review = { md: normalizeUnits(job.markdown), name: job.title || 'Imported recipe', chosen: first ? { [first.i]: true } : {}, cover: first?.i ?? null, hidden: {}, issues: job.issues };
    cur.status = 'ready';
    if (importer.selected === null) importer.selected = id;
    return;
  }
}
