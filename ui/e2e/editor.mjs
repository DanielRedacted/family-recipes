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
// a table (codemirror-markdown-tables: an HTML table with an editor in each cell)
await page.keyboard.press('End');
await page.keyboard.press('Enter');
await page.click('button[aria-label="Insert table"]');
await page.waitForSelector('.cm-formatted .tbl-table');
const cell = (r, c) => page.locator(`.tbl-table .tbl-cell[data-row="${r}"][data-col="${c}"]`);
// a cell's editor mounts just after the click (a person's pause before typing covers it)
const settle = () => page.waitForTimeout(300);
await cell(0, 0).click();
await settle();
await page.keyboard.type('Size');
await page.keyboard.press('Tab');
await page.keyboard.type('Pie');
await page.keyboard.press('Tab');
await page.keyboard.type('9 in');
await page.keyboard.press('Tab');
await page.keyboard.type('1');
assert.equal((await cell(1, 0).innerText()).trim(), '9 in', 'Tab moves to the next cell');
// clicking at the end of a cell's text puts the cursor there
const box = await cell(1, 0).locator('.cm-line, .tbl-cell-view').first().evaluate((el) => {
  const r = document.createRange(); r.selectNodeContents(el); const b = r.getBoundingClientRect(); return { x: b.right - 1, y: b.top + b.height / 2 };
});
await page.mouse.click(box.x, box.y);
await settle();
await page.keyboard.type('ch');
assert.equal((await cell(1, 0).innerText()).trim(), '9 inch', 'click places the cursor where clicked');
// double-click selects exactly the word under the pointer; typing replaces it
await cell(0, 1).click();
await settle();
await cell(0, 1).locator('.cm-line').first().dblclick();
await settle();
assert.ok(await cell(0, 1).locator('.cm-selectionBackground').count() > 0, 'the selection is drawn in the cell');
await page.keyboard.type('Pies');
assert.equal((await cell(0, 1).innerText()).trim(), 'Pies', 'double-click selected exactly the word');
// bold inside a cell: no asterisks shown
await cell(0, 1).locator('.cm-line').first().dblclick();
await settle();
await page.click('button[aria-label="Bold"]');
assert.equal((await cell(0, 1).innerText()).trim(), 'Pies', 'bold shows no asterisks in a cell');
// bold whether the cell is being edited or not (then the package draws it as highlighted text)
const strong = () => cell(0, 1).evaluate((c) => {
  const el = [...c.querySelectorAll('span')].find((x) => x.textContent === 'Pies' && x.offsetParent);
  return el && +getComputedStyle(el).fontWeight >= 600 ? 1 : 0;
});
assert.equal(await strong(), 1, 'cell text is bold');
// undo / redo from inside a cell work on the note
await page.keyboard.press('Meta+z');
await page.waitForTimeout(300);
assert.equal(await strong(), 0, 'undo removes the bold');
await page.keyboard.press('Meta+Shift+z');
await page.waitForTimeout(300);
assert.equal(await strong(), 1, 'redo restores it');
assert.equal((await cell(0, 1).innerText()).trim(), 'Pies', 'no asterisks in a cell that is not being edited');
await page.screenshot({ path: `${out}/table.png` });
await page.click('button:has-text("Save")');
await page.waitForTimeout(800);
let saved = fs.readFileSync(file, 'utf8');
assert.equal(saved, original + '\n**serve warm**\n- [x] buy cream\n### To serve\n\n| Size   | **Pies** |\n| ------ | -------- |\n| 9 inch | 1        |\n', 'saved text is exactly the Markdown typed: ' + JSON.stringify(saved.slice(original.length)));

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

// opening a note with tables changes nothing, so it is not marked as edited
await page.goto(new URL('/#/n/Pies/Pie%20Crust.md?mode=edit', url).href);
await page.waitForSelector('.tbl-table');
await page.waitForTimeout(500);
assert.equal(await page.locator('.save').textContent(), 'Saved', 'opening a table recipe is not an edit');

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('editor ok');
