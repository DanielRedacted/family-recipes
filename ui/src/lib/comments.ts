// Notes on a recipe, each tied to the words highlighted when it was written (Note.svelte, Comments.svelte).
// They live in the recipe's own `comments` property, an app-owned key: written with editProps, so the
// recipe's text and its other properties are never touched. Kept free of Svelte so vitest runs it as is.
import { editProps } from './frontmatter';

export interface Comment {
  id: string;
  /** the highlighted words, spaces squeezed; '' = a note on the whole recipe */
  quote: string;
  /** which time the words appear in the recipe as read (0 = the first), when they appear more than once */
  nth?: number;
  text: string;
  by: string;
  date: string;
  edited_by?: string;
  edited?: string;
}

const str = (v: unknown) => (typeof v === 'string' ? v : v == null ? '' : String(v));

/** The `comments` property as a list, skipping anything that isn't a note. */
export function commentsOf(fm: Record<string, unknown> | null | undefined): Comment[] {
  const list = Array.isArray(fm?.comments) ? fm.comments : [];
  return list
    .filter((c): c is Record<string, unknown> => !!c && typeof c === 'object' && !!str(c.id) && !!str(c.text).trim())
    .map((c) => ({
      id: str(c.id), quote: str(c.quote), text: str(c.text), by: str(c.by), date: str(c.date),
      ...(typeof c.nth === 'number' && c.nth > 0 ? { nth: c.nth } : {}),
      ...(c.edited_by ? { edited_by: str(c.edited_by), edited: str(c.edited) } : {}),
    }));
}

export const newId = () => Math.random().toString(36).slice(2, 8);
export const squeeze = (s: string) => s.replace(/\s+/g, ' ').trim();

/** Add a note (`nth` only when it isn't the first time the words appear). */
export function addComment(raw: string, c: Comment): string {
  const entry: Record<string, unknown> = { id: c.id, quote: c.quote };
  if (c.nth) entry.nth = c.nth;
  Object.assign(entry, { text: c.text, by: c.by, date: c.date });
  return editProps(raw, (doc) => {
    const seq = doc.get('comments');
    if (seq && typeof seq === 'object' && 'add' in (seq as object)) (seq as any).add(doc.createNode(entry));
    else doc.set('comments', [entry]);
  });
}

const indexOf = (doc: any, id: string) => (doc.toJS()?.comments ?? []).findIndex((c: any) => c && String(c.id) === id);

/** Change a note's text, recording who changed it and when. A note that's gone (deleted meanwhile) is left gone. */
export function editComment(raw: string, id: string, text: string, by: string, date: string): string {
  return editProps(raw, (doc) => {
    const i = indexOf(doc, id);
    if (i < 0) return;
    doc.setIn(['comments', i, 'text'], text);
    if (by) { doc.setIn(['comments', i, 'edited_by'], by); doc.setIn(['comments', i, 'edited'], date); }
  });
}

/** Delete a note; the property goes with the last one. */
export function removeComment(raw: string, id: string): string {
  return editProps(raw, (doc) => {
    const i = indexOf(doc, id);
    if (i < 0) return;
    doc.deleteIn(['comments', i]);
    if (!(doc.toJS()?.comments ?? []).length) doc.delete('comments');
  });
}

/** The words as a pattern that matches them across any run of spaces or line breaks. */
const pattern = (quote: string) => new RegExp(squeeze(quote).split(' ').map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('\\s+'), 'g');

/** Where the `nth` time `quote` appears in `text` is, [start, end); the first if there are fewer; null if none. */
export function findQuote(text: string, quote: string, nth = 0): [number, number] | null {
  if (!squeeze(quote)) return null;
  const all = [...text.matchAll(pattern(quote))];
  const m = all[nth] ?? all[0];
  return m ? [m.index!, m.index! + m[0].length] : null;
}

/** Which time `quote` appears when it starts at `offset` in `text` (0 = the first). */
export function nthAt(text: string, quote: string, offset: number): number {
  if (!squeeze(quote)) return 0;
  return [...text.matchAll(pattern(quote))].filter((m) => m.index! < offset).length;
}

// ---------- in the page (browser only) ----------

/** The text nodes under `root`, each with where it starts in their joined text, and that text. */
function texts(root: Element) {
  const nodes: { node: Text; at: number }[] = [];
  let at = 0;
  const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = w.nextNode() as Text | null; n; n = w.nextNode() as Text | null) { nodes.push({ node: n, at }); at += n.data.length; }
  return { nodes, text: nodes.map((x) => x.node.data).join('') };
}

/** Take the highlights out again, leaving the text as it was. */
export function clearMarks(root: Element) {
  for (const m of root.querySelectorAll('mark.cmark')) { const p = m.parentNode!; m.replaceWith(...m.childNodes); p.normalize(); }
}

/** Highlight each note's words in `root` (a <mark> per text node it covers: a range across elements can't be
 *  wrapped whole); where each found note's words start. Only inside the rendered blocks, never their top-level text, which
 *  Svelte's {@html} keeps hold of. */
export function markComments(root: Element, list: Comment[], current: string | null): Map<string, number> {
  clearMarks(root);
  const found = new Map<string, number>();
  for (const c of list) {
    const { nodes, text } = texts(root); // again for each: the last one's marks split text nodes
    const r = findQuote(text, c.quote, c.nth);
    if (!r) continue;
    found.set(c.id, r[0]);
    const parts = nodes.filter((x) => x.node.parentNode !== root && x.at < r[1] && x.at + x.node.data.length > r[0]);
    for (const { node, at } of parts.reverse()) {
      const [a, b] = [Math.max(0, r[0] - at), Math.min(node.data.length, r[1] - at)];
      if (!node.data.slice(a, b).trim()) continue;
      const range = document.createRange();
      range.setStart(node, a);
      range.setEnd(node, b);
      const m = document.createElement('mark');
      m.className = c.id === current ? 'cmark on' : 'cmark';
      m.dataset.id = c.id;
      range.surroundContents(m);
    }
  }
  return found;
}

/** The words selected inside `root`, which time they appear there, and where the selection is on screen. */
export function selectionIn(root: Element): { quote: string; nth: number; rect: DOMRect } | null {
  const sel = getSelection();
  if (!sel || sel.isCollapsed || !sel.rangeCount) return null;
  const range = sel.getRangeAt(0);
  if (!root.contains(range.commonAncestorContainer)) return null;
  const quote = squeeze(range.toString()).slice(0, 500); // the page's text, as the highlights search it
  if (!quote) return null;
  const before = document.createRange();
  before.selectNodeContents(root);
  before.setEnd(range.startContainer, range.startOffset);
  return { quote, nth: nthAt(texts(root).text, quote, before.toString().length), rect: range.getBoundingClientRect() };
}
