// The contents file and the recipe book: node e2e/book.mjs <admin-url> <vault> <out>
// README.md at the top of the recipes lists every recipe and follows renames, but is never a recipe; the
// recipe book saves as one self-contained web page, and its print layout starts each folder on a new page.
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const at = (p) => path.join(vault, p);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, ok, ms = 8000) {
  for (const end = Date.now() + ms; Date.now() < end; await sleep(200)) if (await ok()) return;
  assert.fail(`timed out waiting for ${what}`);
}
fs.mkdirSync(out, { recursive: true });

// a recipe with a picture and links to another recipe and to a folder
fs.mkdirSync(at('attachments'), { recursive: true });
fs.writeFileSync(at('attachments/book-test.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64'));
fs.mkdirSync(at('Pies'), { recursive: true });
fs.writeFileSync(at('Pies/Picture Pie.md'), '---\nservings: 8\nprep_time: 20 min\n---\n\n# Picture Pie\n\n![The pie](/attachments/book-test.png)\n\n## Ingredients\n\n- 1 [[Picture Crust]]\n- 2 cups apples\n\n## Directions\n\n1. Bake, as in [the cakes](</Cakes>).\n');
fs.writeFileSync(at('Pies/Picture Crust.md'), '# Picture Crust\n\n## Ingredients\n\n- 1 cup flour\n\n## Directions\n\n1. Roll.\n');

const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1280, height: 860 }, acceptDownloads: true });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto(url);
await page.request.put(new URL('/api/settings', url).href, { headers: { 'X-Recipes': '1', Origin: new URL(url).origin }, data: { setup_done: true } });
await page.reload();
await page.waitForSelector('a.folder');

// ---------- README.md ----------
const readme = () => (fs.existsSync(at('README.md')) ? fs.readFileSync(at('README.md'), 'utf8') : '');
await until('README.md with the new recipes', () => readme().includes('[Picture Pie](Pies/Picture%20Pie.md)'));
const links = [...readme().matchAll(/\]\(([^)]+)\)/g)].map((m) => decodeURIComponent(m[1]));
const notes = (await (await page.request.get(new URL('/api/vault', url).href)).json()).files.filter((f) => f.kind === 'note' && !f.path.startsWith('attachments/'));
assert.equal(links.length, notes.length, 'README lists every recipe once');
for (const l of links) assert.ok(fs.existsSync(at(l)), `README link ${l} resolves`);
assert.ok(!notes.some((f) => f.path === 'README.md'), 'README.md is not a recipe');
await page.keyboard.press('Meta+k');
await page.fill('input[aria-label="Search"]', 'README');
await sleep(500);
assert.equal(await page.locator('li[role=option]:has-text("README")').count(), 0, 'README.md not in search');
await page.keyboard.press('Escape');

// a rename through the app moves its line; with nothing changed the file is left alone
await page.request.post(new URL('/api/fs/move', url).href, { headers: { 'X-Recipes': '1', Origin: new URL(url).origin }, data: { from: 'Pies/Picture Crust.md', to: 'Pies/Picture Pastry.md' } });
await until('README.md after the rename', () => readme().includes('(Pies/Picture%20Pastry.md)') && !readme().includes('Picture%20Crust'));
fs.writeFileSync(at('Pies/Picture Pie.md'), fs.readFileSync(at('Pies/Picture Pie.md'), 'utf8').replace('[[Picture Crust]]', '[[Picture Pastry]]'));
await sleep(1500);
const settled = fs.statSync(at('README.md')).mtimeMs;
fs.writeFileSync(at('Pies/Picture Pie.md'), fs.readFileSync(at('Pies/Picture Pie.md'), 'utf8') + '\n'); // a change that moves no line
await sleep(1500);
assert.equal(fs.statSync(at('README.md')).mtimeMs, settled, 'README.md not rewritten when nothing in it changed');

// ---------- Settings → Recipe book ----------
await page.goto(new URL('/#/settings/general', url).href);
await page.waitForSelector('h2:has-text("Recipe book")');
for (const [w, h, name] of [[1280, 860, 'desktop'], [390, 844, 'phone']]) {
  await page.setViewportSize({ width: w, height: h });
  await page.locator('h2:has-text("Recipe book")').scrollIntoViewIfNeeded();
  await page.screenshot({ path: `${out}/settings-${name}.png` });
}
await page.setViewportSize({ width: 1280, height: 860 });

const [dl] = await Promise.all([page.waitForEvent('download'), page.click('button:has-text("Save as web page")')]);
assert.equal(dl.suggestedFilename(), 'Recipes.html');
const file = `${out}/Recipes.html`;
await dl.saveAs(file);
const html = fs.readFileSync(file, 'utf8');
assert.ok(!html.includes('/api/files/') && !html.includes('href="#/'), 'no links back into the app');
assert.ok(html.includes('src="data:image/png;base64,'), 'the picture is inside the file');

const book = await ctx.newPage();
book.on('pageerror', (e) => errors.push('book: ' + e.message));
await book.goto('file://' + file);
const facts = await book.evaluate(() => ({
  articles: document.querySelectorAll('article.prose').length,
  chapters: document.querySelectorAll('section.chapter').length,
  broken: [...document.querySelectorAll('a[href^="#"]')].map((a) => a.getAttribute('href')).filter((h) => !document.getElementById(h.slice(1))),
}));
assert.equal(facts.articles, notes.length, 'every recipe is in the book');
assert.deepEqual(facts.broken, [], 'every link inside the book leads somewhere');
const pie = await book.evaluate(() => {
  const a = [...document.querySelectorAll('article')].find((x) => x.querySelector('h1')?.textContent === 'Picture Pie');
  return { details: a.querySelector('dl.details')?.textContent, folderLink: a.innerHTML.includes('the cakes</a>'), crust: a.querySelector('a[href^="#book-r"]')?.textContent };
});
assert.equal(pie.crust, 'Picture Pastry', 'a link to another recipe points into the book');
assert.match(pie.details, /Servings\s*8/);
assert.equal(pie.folderLink, false, 'a link to a folder is plain text');
for (const [w, h, name] of [[1280, 860, 'desktop'], [390, 844, 'phone']]) {
  await book.setViewportSize({ width: w, height: h });
  assert.ok(await book.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `no sideways scroll (${name})`);
  await book.screenshot({ path: `${out}/book-${name}.png` });
  await book.evaluate(() => document.querySelector('.chapter').scrollIntoView());
  await book.screenshot({ path: `${out}/book-${name}-chapter.png` });
  await book.evaluate(() => scrollTo(0, 0));
}

// ---------- print layout, in the app's own page ----------
await page.evaluate(() => { window.print = () => { window.__printed = true; }; });
await page.click('button:has-text("Print or save as PDF")');
await page.waitForFunction(() => window.__printed === true);
assert.equal(await page.evaluate(() => document.body.classList.contains('printing-book') && !!document.querySelector('body > .book')), true);
await page.pdf({ path: `${out}/Recipes.pdf`, format: 'Letter', margin: { top: '0.5in', bottom: '0.5in', left: '0.5in', right: '0.5in' } });
await page.evaluate(() => dispatchEvent(new Event('afterprint')));
assert.equal(await page.evaluate(() => !!document.querySelector('body > .book') || document.body.classList.contains('printing-book')), false, 'the book leaves the page after printing');

assert.deepEqual(errors, []);
await browser.close();
console.log(`book ok: ${facts.articles} recipes in ${facts.chapters} chapters; README lists ${links.length}`);
