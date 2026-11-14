import { api } from './api';

export interface ImageCand { i: number; alt: string; kind: string; step?: string | null; url: string }
export interface Issue { level: 'warn' | 'info'; message: string }
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

/** A job started elsewhere (e.g. "Convert this PDF" in the viewer) for the Import page to open. */
export const importer = $state({ pending: null as ImportJob | null });

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
