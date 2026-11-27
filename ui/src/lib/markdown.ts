import MarkdownIt from 'markdown-it';
import DOMPurify from 'dompurify';
import { fileUrl } from './api';
import { app, noteHash, resolveLink, dirOf, stemOf } from './state.svelte';
import { groupLines, scaleLine, boldQty, inVersion, solidGramsPerMl, toCelsius, versionsOf, type Units, type Weigh } from './scale';
import { foods, catalog, loadIngredients } from './ingredients.svelte';
import { aliasesOf } from './search';
import { authorsOf } from './credit';
import { personHash, peopleNames, key } from './people';
import type { Entry, Person } from './types';

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
  const href = f ? noteHash(f.path) : personOrIngredient(target);
  if (!href) return `<a class="wikilink missing" title="No recipe, person or ingredient called “${esc(target)}”">${esc(label)}</a>`;
  return `<a class="wikilink" href="${esc(href)}">${esc(label)}</a>`;
};

// ---------- @person and #ingredient: only a name someone or something in the book goes by becomes a link ----------
let named: { files: Entry[]; people: Person[]; map: Map<string, string> } | null = null;
const people = () => {
  if (named?.files !== app.files || named.people !== app.people) named = { files: app.files, people: app.people, map: peopleNames(app.files, app.people) };
  return named.map;
};
let stocked: { items: unknown; map: Map<string, string> } | null = null;
const ingredients = () => {
  if (!catalog.loaded) loadIngredients(); // a page that read before it arrived renders again then
  if (stocked?.items !== catalog.items) {
    const map = new Map<string, string>();
    for (const i of catalog.items) for (const n of [i.name, ...i.forms]) if (!map.has(key(n))) map.set(key(n), i.name);
    stocked = { items: catalog.items, map };
  }
  return stocked.map;
};
const ingredientHash = (name: string) => '#/ingredients/' + encodeURIComponent(name);
function personOrIngredient(name: string): string | null {
  const p = people().get(key(name));
  if (p) return personHash(p);
  const i = ingredients().get(key(name));
  return i ? ingredientHash(i) : null;
}

const WORD = /[\p{L}\p{N}_]/u;
/** The longest of `names` that `src` starts with and that ends at a word boundary: [length, page name]. */
// ponytail: a linear scan per @ or #, fine for one household's names and ingredients; a prefix map if it ever shows
function longest(src: string, names: Map<string, string>): [number, string] | null {
  const low = src.toLowerCase().replace(/\s+/g, ' ');
  let best: [number, string] | null = null;
  for (const [k, v] of names) if (k.length > (best?.[0] ?? 0) && low.startsWith(k) && !WORD.test(low[k.length] ?? '')) best = [k.length, v];
  return best;
}

md.inline.ruler.before('link', 'mention', (state, silent) => {
  const c = state.src[state.pos];
  if ((c !== '@' && c !== '#') || state.linkLevel > 0) return false; // never a link inside a link's text
  if (state.pos > 0 && (WORD.test(state.src[state.pos - 1]) || /[@#&]/.test(state.src[state.pos - 1]))) return false; // a@b.com, C#, &#39;
  const rest = state.src.slice(state.pos + 1);
  const hit = longest(rest, c === '@' ? people() : ingredients());
  if (!hit) return false;
  // the length is in squeezed text: walk the original the same number of squeezed characters
  let n = 0, seen = 0;
  while (seen < hit[0] && n < rest.length) { if (/\s/.test(rest[n])) { while (/\s/.test(rest[n + 1] ?? '')) n++; } n++; seen++; }
  if (!silent) {
    const t = state.push('mention', '', 0);
    t.meta = { href: c === '@' ? personHash(hit[1]) : ingredientHash(hit[1]), label: rest.slice(0, n) };
  }
  state.pos += 1 + n;
  return true;
});
md.renderer.rules.mention = (tokens, idx) => {
  const { href, label } = tokens[idx].meta as { href: string; label: string };
  return `<a class="wikilink mention" href="${md.utils.escapeHtml(href)}">${md.utils.escapeHtml(label)}</a>`;
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

/** Metric mode's grams for solids, from the nutrition table (empty until `loadFoods` has run). */
export const weigh: Weigh = (a) => solidGramsPerMl(a, foods.table);

/**
 * Raw note text with ingredient quantities scaled/converted in place. A table with alternative
 * portions becomes a list of the chosen portion's lines (see `groupLines`). Metric: solids in grams
 * once the nutrition table is loaded, temperatures in °C. `version`: in a recipe with versions, the
 * other versions' lines are left out (what comes before the first version stays).
 */
export function scaledRaw(e: Entry, factor: number, units: Units, portion = 0, version?: number): string {
  const raw = e.raw ?? '';
  if (!e.recipe) return raw;
  let lines = raw.split('\n');
  for (const g of e.recipe.ingredients) {
    if (g.table) {
      const [a, b] = g.table;
      const list = groupLines(g, portion, factor, units, weigh).map((it) => '- ' + boldQty(autoLink(it.text, e.path)));
      for (let k = a; k <= b; k++) lines[k] = '';
      lines[a] = (a > 0 && lines[a - 1].trim() ? '\n' : '') + list.join('\n');
      continue;
    }
    for (const it of g.items) {
      const l = lines[it.line];
      if (l === undefined) continue;
      const w = units === 'metric' ? weigh(it.amounts?.[0]) : undefined;
      lines[it.line] = l.replace(/^(\s*(?:(?:[-*+]|\d+[.)])\s+)*)(.*)$/, (_, mk: string, rest: string) => mk + boldQty(autoLink(scaleLine(rest, factor, units, w), e.path)));
    }
  }
  const vs = versionsOf(e);
  if (version !== undefined && vs[version]) lines = lines.map((l, k) => (k < vs[0].from || inVersion(k, vs[version]) ? l : ''));
  if (units === 'metric') lines = lines.map((l, k) => (k < e.body_line ? l : toCelsius(l)));
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

/** The recipe an ingredient line uses as an ingredient: one it links to, else one it names (`autoMatch`). */
export function subRecipeOf(text: string, from: Entry): Entry | null {
  const wiki = text.match(/\[\[([^\]|#\n]+)/);
  const link = text.match(/\]\(<?([^)>\s]+)>?\)/);
  const path = wiki ? resolveLink(wiki[1].trim())?.path
    : link ? vaultTarget(link[1], dirOf(from.path))
    : autoMatch(text, from.path)?.path;
  const e = path ? app.byPath[path] : undefined;
  return e?.recipe && e.path !== from.path ? e : null;
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

export const escHtml = (t: string) => t.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);

/**
 * A recipe as the reader shows it: scaled and converted, with "By …" under its title (each name a link to
 * their page). `withTitle` adds the title as a heading when the file has none (the recipe book).
 */
export function noteHtml(e: Entry, factor: number, units: Units, portion = 0, withTitle = false, version?: number): string {
  let out = renderMarkdown(bodyOf(scaledRaw(e, factor, units, portion, version), e.body_line), e.path);
  if (withTitle && !/<h1[\s>]/.test(out)) out = `<h1>${escHtml(e.title)}</h1>\n` + out;
  const names = authorsOf(e.frontmatter?.author);
  if (!names.length) return out;
  const by = new Intl.ListFormat(undefined, { type: 'conjunction' }).formatToParts(names)
    .map((p) => (p.type === 'element' ? `<a href="${escHtml(personHash(p.value))}">${escHtml(p.value)}</a>` : escHtml(p.value))).join('');
  return out.replace('</h1>', () => `</h1><p class="byline">By ${by}</p>`);
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
