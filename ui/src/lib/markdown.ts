import MarkdownIt from 'markdown-it';
import DOMPurify from 'dompurify';
import { fileUrl } from './api';
import { app, noteHash, resolveLink, dirOf, stemOf } from './state.svelte';
import { groupLines, scaleLine, boldQty, type Units } from './scale';
import { aliasesOf } from './search';
import type { Entry } from './types';

const md = new MarkdownIt({ html: true, linkify: true, typographer: false });

/** Resolve a link/src written in a note to a vault path, or null for external URLs. */
export function vaultTarget(href: string, noteDir: string): string | null {
  if (!href || /^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('//') || href.startsWith('#')) return null;
  let p = decodeURI(href.replace(/^<|>$/g, '').split('#')[0].split('?')[0]);
  if (!p.startsWith('/')) p = (noteDir ? noteDir + '/' : '') + p;
  const out: string[] = [];
  for (const seg of p.split('/')) {
    if (!seg || seg === '.') continue;
    if (seg === '..') out.pop();
    else out.push(seg);
  }
  return out.join('/');
}

// [[Target]] / [[Target|label]] / ![[file.ext]]
md.inline.ruler.before('link', 'wikilink', (state, silent) => {
  const src = state.src.slice(state.pos);
  const m = src.match(/^(!?)\[\[([^\]|#\n]+)(?:#[^\]|\n]*)?(?:\|([^\]\n]+))?\]\]/);
  if (!m) return false;
  if (!silent) {
    const t = state.push('wikilink', '', 0);
    t.meta = { embed: m[1] === '!', target: m[2].trim(), label: (m[3] || m[2]).trim() };
  }
  state.pos += m[0].length;
  return true;
});

md.renderer.rules.wikilink = (tokens, idx) => {
  const { embed, target, label } = tokens[idx].meta as any;
  const esc = md.utils.escapeHtml;
  if (embed && /\.(png|jpe?g|gif|webp|avif)$/i.test(target)) {
    const f = resolveLink(target) ?? { path: target };
    return `<img src="${esc(fileUrl(f.path))}" alt="${esc(label)}">`;
  }
  const f = resolveLink(target);
  if (!f) return `<a class="wikilink missing" title="No note named “${esc(target)}”">${esc(label)}</a>`;
  return `<a class="wikilink" href="${esc(noteHash(f.path))}">${esc(label)}</a>`;
};

const defaultImage = md.renderer.rules.image!;
md.renderer.rules.image = (tokens, idx, opts, env, self) => {
  const t = tokens[idx];
  const p = vaultTarget(String(t.attrGet('src') || ''), String(env?.dir ?? ''));
  if (p !== null) t.attrSet('src', fileUrl(p));
  t.attrSet('loading', 'lazy');
  return defaultImage(tokens, idx, opts, env, self);
};

md.renderer.rules.link_open = (tokens, idx, opts, _env, self) => {
  const t = tokens[idx];
  const href = String(t.attrGet('href') || '');
  const p = vaultTarget(href, String(_env?.dir ?? ''));
  if (p !== null) {
    t.attrSet('href', noteHash(p));
    t.attrJoin('class', 'internal');
  } else if (/^https?:/i.test(href)) {
    // a web page, not a recipe: marked with ↗ and opened in the browser
    t.attrSet('target', '_blank');
    t.attrSet('rel', 'noopener noreferrer');
    t.attrJoin('class', 'ext');
    try { t.attrSet('title', `Opens ${new URL(href).hostname.replace(/^www\./, '')} in your browser`); } catch {}
  }
  return self.renderToken(tokens, idx, opts);
};

md.renderer.rules.table_open = () => '<div class="table-wrap"><table>\n';
md.renderer.rules.table_close = () => '</table></div>\n';

/**
 * Raw note text with ingredient quantities scaled/converted in place. A table with alternative
 * portions becomes a list of the chosen portion's lines (see `groupLines`).
 */
export function scaledRaw(e: Entry, factor: number, units: Units, portion = 0): string {
  const raw = e.raw ?? '';
  if (!e.recipe) return raw;
  const lines = raw.split('\n');
  for (const g of e.recipe.ingredients) {
    if (g.table) {
      const [a, b] = g.table;
      const list = groupLines(g, portion, factor, units).map((it) => '- ' + boldQty(autoLink(it.text, e.path)));
      for (let k = a; k <= b; k++) lines[k] = '';
      lines[a] = (a > 0 && lines[a - 1].trim() ? '\n' : '') + list.join('\n');
      continue;
    }
    for (const it of g.items) {
      const l = lines[it.line];
      if (l === undefined) continue;
      lines[it.line] = l.replace(/^(\s*(?:(?:[-*+]|\d+[.)])\s+)*)(.*)$/, (_, mk: string, rest: string) => mk + boldQty(autoLink(scaleLine(rest, factor, units), e.path)));
    }
  }
  return lines.join('\n');
}

// ---------- ingredients that are recipes of their own ----------

// ponytail: names are matched as written, so only multi-word names and Condiments recipes take part
// (a lone "Soda" or "Turkey" would link "baking soda" and "ground turkey"); links written in the file win.
let names: { files: unknown; list: { rx: RegExp; path: string }[] } = { files: null, list: [] };
function recipeNames() {
  if (names.files === app.files) return names.list;
  const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const list = app.files
    .filter((f) => f.kind === 'note' && f.recipe)
    .flatMap((f) => [stemOf(f.path), ...aliasesOf(f)].map((n) => ({ n: n.trim(), path: f.path, cond: dirOf(f.path).split('/')[0] === 'Condiments' })))
    .filter((x) => x.n.length >= 4 && (x.cond || /\s/.test(x.n)))
    .sort((a, b) => b.n.length - a.n.length)
    .map((x) => ({ rx: new RegExp(`(?<![\\w\\[])${esc(x.n)}(?![\\w\\]])`, 'i'), path: x.path }));
  names = { files: app.files, list };
  return list;
}

/** The recipe an ingredient line names (not `self`), and where; null when the line has a link already. */
export function autoMatch(text: string, self: string): { path: string; index: number; len: number } | null {
  if (/\]\(|\[\[/.test(text)) return null;
  for (const { rx, path } of recipeNames()) {
    if (path === self) continue;
    const m = text.match(rx);
    if (m) return { path, index: m.index!, len: m[0].length };
  }
  return null;
}

/** "½ cup mayonnaise" → "½ cup [mayonnaise](#/n/Condiments/Mayonnaise.md)" when another recipe has that name. */
export function autoLink(text: string, self: string): string {
  const m = autoMatch(text, self);
  if (!m) return text;
  const end = m.index + m.len;
  return `${text.slice(0, m.index)}[${text.slice(m.index, end)}](${noteHash(m.path)})${text.slice(end)}`;
}

/** `f` links to `path`: a [[wiki link]], a Markdown link to the file, or an ingredient named after it. */
export function linksTo(f: Entry, path: string): boolean {
  if (f.links?.some((l) => resolveLink(l)?.path === path)) return true;
  for (const m of (f.raw ?? '').matchAll(/\]\(<?([^)>\s]+)>?\)/g)) if (vaultTarget(m[1], dirOf(f.path)) === path) return true;
  return !!f.recipe?.ingredients.some((g) => g.items.some((i) => autoMatch(i.text, f.path)?.path === path));
}

export function bodyOf(raw: string, bodyLine: number): string {
  return bodyLine ? raw.split('\n').slice(bodyLine).join('\n') : raw;
}

export function renderMarkdown(body: string, notePath: string): string {
  const html = md.render(body, { dir: dirOf(notePath) });
  return DOMPurify.sanitize(html, {
    ADD_ATTR: ['target'],
    FORBID_TAGS: ['style', 'form', 'input', 'button', 'iframe', 'object', 'embed', 'textarea', 'select'],
  });
}

export function renderInline(text: string): string {
  return DOMPurify.sanitize(md.renderInline(text, { dir: '' }));
}
