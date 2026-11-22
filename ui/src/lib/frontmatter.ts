// Frontmatter edits that keep the file's comments, order and block style (yaml's Document API).
// The note body is never re-serialised.
import { Document, parseDocument } from 'yaml';

/** Split raw text into [yaml, body]; yaml is '' when there is no frontmatter. */
export function splitRaw(raw: string): [string, string] {
  const m = raw.match(/^---\r?\n([\s\S]*?\r?\n)?---[ \t]*(?:\r?\n|$)/);
  return m ? [m[1] ?? '', raw.slice(m[0].length)] : ['', raw];
}

/** Change the frontmatter with `edit`; an empty result drops the block. */
export function editProps(raw: string, edit: (doc: Document) => void): string {
  const [yaml, body] = splitRaw(raw);
  // a new block-style map for a note without properties ("favorite: true", not "{ favorite: true }")
  const doc = yaml.trim() ? parseDocument(yaml) : new Document({});
  edit(doc);
  const out = doc.contents && (doc.contents as any).items?.length ? String(doc) : '';
  const fm = out && out.trim() !== '{}' ? `---\n${out.endsWith('\n') ? out : out + '\n'}---\n` : '';
  return fm + (fm && body && !body.startsWith('\n') ? '\n' : '') + body;
}

/** Set (or with value===undefined delete) one frontmatter key, keeping comments and order. */
export function setProp(raw: string, key: string, value: unknown): string {
  return editProps(raw, (doc) => {
    if (value === undefined || value === '' || (Array.isArray(value) && !value.length)) doc.delete(key);
    else doc.set(key, value);
  });
}
