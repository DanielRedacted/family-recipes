// Formatted editor: node e2e/editor.mjs <admin-url> <vault> <out>
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1280, height: 860 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
const file = `${vault}/Pies/Pumpkin Pie.md`;
const original = fs.readFileSync(file, 'utf8');

await page.goto(url);
await page.waitForSelector('a.folder');
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md?mode=edit', url).href);
await page.waitForSelector('.cm-formatted .cm-lp-h2');

// markup is never shown, even on the cursor's line
const h2 = page.locator('.cm-line.cm-lp-h2').first();
assert.equal((await h2.textContent()).trim(), 'Ingredients', 'heading marks hidden');
assert.ok(await page.locator('.cm-lp-bullet').count() > 5, 'bullets drawn as dots');
await h2.click();
assert.equal((await h2.textContent()).trim(), 'Ingredients', 'heading marks stay hidden on the active line');
assert.equal(await page.locator('select[aria-label="Text type"]').inputValue(), 'h2', 'text type shows Heading');
await page.screenshot({ path: `${out}/formatted.png` });

// toolbar: bold a word, add a checklist line; the file gets exactly the typed Markdown
await page.click('.cm-content');
await page.keyboard.press('Meta+End');
await page.keyboard.type('\nserve warm');
await page.keyboard.press('Shift+Home');
await page.click('button[aria-label="Bold"]');
const last = page.locator('.cm-line').filter({ hasText: 'serve warm' });
assert.equal((await last.textContent()).trim(), 'serve warm', 'no asterisks shown');
await page.keyboard.press('End');
await page.keyboard.type('\nbuy cream');
await page.click('button[aria-label="Checklist"]');
await page.waitForSelector('input.cm-lp-task');
await page.click('input.cm-lp-task');
// text type: make a line a Subheading, then back to Body
await page.keyboard.press('End');
await page.keyboard.type('\nTo serve');
await page.selectOption('select[aria-label="Text type"]', 'h3');
await page.waitForSelector('.cm-line.cm-lp-h3');
// a table: insert, type, Tab between cells, add a column
await page.keyboard.press('End');
await page.keyboard.press('Enter');
await page.click('button[aria-label="Insert table"]');
await page.waitForSelector('.cm-tbl-row');
await page.keyboard.type('Size');
await page.keyboard.press('Tab');
await page.keyboard.type('Pie');
await page.keyboard.press('Tab');
await page.keyboard.type('9 in');
await page.keyboard.press('Tab');
await page.keyboard.type('1');
await page.click('button[aria-label="Add column"]');
await page.keyboard.type('note');
assert.equal(await page.locator('.cm-tbl-row').count(), 2, 'table rows are grids');
assert.equal(await page.locator('.cm-tbl-row').first().locator('.cm-tbl-cell').count(), 3, 'three columns after Add column');
assert.ok(!(await page.locator('.cm-tbl-row').allTextContents()).some((t) => t.includes('|')), 'no pipes shown');
await page.screenshot({ path: `${out}/table.png` });
await page.click('button:has-text("Save")');
await page.waitForTimeout(800);
let saved = fs.readFileSync(file, 'utf8');
assert.equal(saved, original + '\n**serve warm**\n- [x] buy cream\n### To serve\n\n| Size | Pie |  |\n| --- | --- | --- |\n| 9 in | 1 | note |', 'saved text is exactly the Markdown typed');

// Backspace at the start of bold text removes both hidden ** marks (no stray asterisks)
await last.click();
await page.keyboard.press('Home');
await page.keyboard.press('Backspace');
await page.click('button:has-text("Save")');
await page.waitForTimeout(800);
saved = fs.readFileSync(file, 'utf8');
assert.ok(saved.includes('\nserve warm\n- [x] buy cream'), 'bold removed cleanly: ' + JSON.stringify(saved.slice(original.length - 5, original.length + 40)));
await page.screenshot({ path: `${out}/formatted-after.png` });

// Markdown (source) view shows everything
await page.click('button:has-text("Markdown")');
assert.equal((await page.locator('.cm-line').filter({ hasText: 'Ingredients' }).first().textContent()).trim(), '## Ingredients');
await page.click('button:has-text("Formatted")');

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('editor ok');
