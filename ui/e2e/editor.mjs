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

// markup hidden away from the cursor, shown on the cursor's line
const h2 = page.locator('.cm-line.cm-lp-h2').first();
assert.equal((await h2.textContent()).trim(), 'Ingredients', 'heading marks hidden');
assert.ok(await page.locator('.cm-lp-bullet').count() > 5, 'bullets drawn as dots');
await h2.click();
assert.ok((await h2.textContent()).startsWith('## '), 'heading marks shown on the active line');
await page.screenshot({ path: `${out}/formatted.png` });

// toolbar: bold a word, add a checklist line; the file gets exactly the typed Markdown
await page.click('.cm-content');
await page.keyboard.press('Meta+End');
await page.keyboard.type('\nserve warm');
await page.keyboard.press('Shift+Home');
await page.click('button[aria-label="Bold"]');
await page.keyboard.press('End');
await page.keyboard.type('\nbuy cream');
await page.click('button[aria-label="Checklist"]');
await page.click('.cm-line.cm-lp-h2 >> nth=0'); // move the cursor away so the box renders
await page.waitForSelector('input.cm-lp-task');
await page.click('input.cm-lp-task');
await page.click('button:has-text("Save")');
await page.waitForTimeout(800);
const saved = fs.readFileSync(file, 'utf8');
assert.equal(saved, original + '\n**serve warm**\n- [x] buy cream', 'saved text is exactly the Markdown typed');
await page.screenshot({ path: `${out}/formatted-after.png` });

// Markdown (source) view shows everything
await page.click('button:has-text("Markdown")');
assert.equal((await page.locator('.cm-line').filter({ hasText: 'Ingredients' }).first().textContent()).trim(), '## Ingredients');
await page.click('button:has-text("Formatted")');

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('editor ok');
