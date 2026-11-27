// The recipe book (Settings → General → Recipe book): every recipe in one document, a chapter per folder,
// with a title page and contents. Rendered here with the reader's own renderer (noteHtml), then saved as
// one self-contained web page, or put into this page and printed (to a PDF in the desktop app).
import { app, prefs, dirOf, cookbookName } from './state.svelte';
import { api } from './api';
import { accentCss } from './accent';
import { noteHtml, escHtml } from './markdown';
import { loadFoods } from './ingredients.svelte';
import { LABELS, HIDDEN, fmtVal } from '../components/Properties.svelte';
import type { Entry } from './types';
import css from '../prose.css?raw';

export interface Chapter { name: string; recipes: Entry[] }

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

/** Folders by name (each followed by its subfolders), recipes by title; notes at the top come last. */
export function bookOrder(files: Entry[]): Chapter[] {
  const by = new Map<string, Entry[]>();
  for (const f of files) {
    if (f.kind !== 'note' || f.path.startsWith('attachments/')) continue;
    const d = dirOf(f.path);
    if (!by.has(d)) by.set(d, []);
    by.get(d)!.push(f);
  }
  return [...by]
    .sort(([a], [b]) => (a === '' ? 1 : b === '' ? -1 : collator.compare(a, b)))
    .map(([d, recipes]) => ({
      name: d ? d.split('/').join(' / ') : 'Other recipes',
      recipes: recipes.sort((x, y) => collator.compare(x.title, y.title) || collator.compare(x.path, y.path)),
    }));
}

/** Let the browser draw and answer taps between recipes (scheduler.yield where there is one). */
const pause = (): Promise<void> => {
  const s = (globalThis as any).scheduler;
  return s?.yield ? s.yield() : new Promise((r) => setTimeout(r, 0));
};

/** Yield, times, source…: the details the reader shows in its row, under the title. */
function addDetails(article: HTMLElement, r: Entry) {
  const fm = r.frontmatter ?? {};
  const keys = Object.keys(fm).filter((k) => !HIDDEN.includes(k) && fmtVal(fm[k]).trim());
  if (!keys.length) return;
  const dl = document.createElement('dl');
  dl.className = 'details';
  for (const k of keys) {
    const row = document.createElement('div');
    const dt = document.createElement('dt');
    const dd = document.createElement('dd');
    dt.textContent = LABELS[k] ?? k.replaceAll('_', ' ');
    dd.textContent = fmtVal(fm[k]);
    row.append(dt, dd);
    dl.append(row);
  }
  const after = article.querySelector('h1 + .byline') ?? article.querySelector('h1');
  if (after) after.after(dl);
  else article.prepend(dl);
}

/** A link to a recipe in the book points at it in the book; other app links (folders, people, files) become text. */
function relink(book: HTMLElement, ids: Map<string, string>) {
  for (const a of book.querySelectorAll<HTMLAnchorElement>('a[href^="#/"]')) {
    const href = a.getAttribute('href')!;
    const path = href.startsWith('#/n/') ? href.slice(4).split('?')[0].split('/').map(decodeURIComponent).join('/') : '';
    const id = ids.get(path);
    if (id) a.setAttribute('href', '#' + id);
    else a.replaceWith(...a.childNodes);
  }
}

async function buildBook(): Promise<HTMLElement> {
  // Metric weighs solids with the nutrition table; offline, they stay in ml
  if (prefs.units === 'metric') await loadFoods().catch(() => {});
  const chapters = bookOrder(app.files);
  const ids = new Map<string, string>();
  for (const c of chapters) for (const r of c.recipes) ids.set(r.path, `book-r${ids.size + 1}`);
  const book = document.createElement('div');
  book.className = 'book';
  const date = new Date().toLocaleDateString(undefined, { dateStyle: 'long' });
  const count = ids.size === 1 ? '1 recipe' : `${ids.size} recipes`;
  book.innerHTML =
    `<section class="cover"><h1>${escHtml(cookbookName())}</h1><p>${count}, saved ${escHtml(date)}</p></section>` +
    `<section class="toc"><h2>Contents</h2><div class="toc-cols">` +
    chapters.map((c, i) => `<div class="toc-group"><h3><a href="#book-c${i + 1}">${escHtml(c.name)}</a></h3><ul>` +
      c.recipes.map((r) => `<li><a href="#${ids.get(r.path)}">${escHtml(r.title)}</a></li>`).join('') + '</ul></div>').join('') +
    '</div></section>';
  let deadline = performance.now() + 50;
  for (const [i, c] of chapters.entries()) {
    const sec = document.createElement('section');
    sec.className = 'chapter';
    sec.id = `book-c${i + 1}`;
    sec.innerHTML = `<h1 class="chapter-title">${escHtml(c.name)}</h1>`;
    for (const r of c.recipes) {
      const article = document.createElement('article');
      article.className = 'prose';
      article.id = ids.get(r.path)!;
      article.innerHTML = noteHtml(r, 1, prefs.units, 0, true);
      addDetails(article, r);
      sec.append(article);
      if (performance.now() >= deadline) {
        await pause();
        deadline = performance.now() + 50;
      }
    }
    book.append(sec);
  }
  relink(book, ids);
  // every picture now, not when scrolled to: a printed page never scrolls
  for (const img of book.querySelectorAll('img')) img.removeAttribute('loading');
  return book;
}

const dataUrl = (b: Blob) => new Promise<string>((ok, fail) => {
  const r = new FileReader();
  r.onload = () => ok(String(r.result));
  r.onerror = () => fail(r.error);
  r.readAsDataURL(b);
});

/** The book as one web page that needs nothing else: pictures inside it, the app's recipe styles in it. */
export async function bookHtml(): Promise<string> {
  const book = await buildBook();
  for (const img of book.querySelectorAll('img')) {
    const src = img.getAttribute('src') ?? '';
    if (!src.startsWith('/api/')) continue;
    try {
      const r = await fetch(src);
      if (!r.ok) throw new Error();
      img.src = await dataUrl(await r.blob());
    } catch { img.removeAttribute('src'); } // its alt text stays
  }
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escHtml(cookbookName())}</title>
<style>
${css}
${accentCss(prefs.accent || app.session?.accent || null, true)}
</style>
</head>
<body class="book-file">
${book.outerHTML}
</body>
</html>
`;
}

/** Save the web page: to Downloads by the desktop app (returns the file name), else as a browser download. */
export async function saveBookHtml(): Promise<string | null> {
  const html = await bookHtml();
  if (app.session.shell) return (await api<{ name: string }>('POST', '/api/shell/save-html', { html, name: cookbookName() })).name;
  const url = URL.createObjectURL(new Blob([html], { type: 'text/html' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = `${cookbookName()}.html`;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
  return null;
}

/** Take the book out of the page (also done before any other print). */
export function clearBook() {
  document.body.classList.remove('printing-book');
  document.querySelector('body > .book')?.remove();
}

/**
 * Print the book: put it into this page, shown only on paper (prose.css), then print. The desktop app
 * writes the PDF to Downloads without a dialog (returns the file name); a browser opens its print dialog.
 */
export async function printBook(): Promise<string | null> {
  clearBook();
  const book = await buildBook();
  document.body.append(book);
  await Promise.all([...book.querySelectorAll('img')].map((i) => i.decode().catch(() => {})));
  document.body.classList.add('printing-book');
  if (app.session.shell) {
    try { return (await api<{ name: string }>('POST', '/api/shell/save-pdf', { name: cookbookName() })).name; }
    finally { clearBook(); }
  }
  addEventListener('afterprint', clearBook, { once: true });
  window.print();
  return null;
}
