// End-to-end flows against a running server: node e2e/flows.mjs <admin-url> <vault-dir> <outdir>
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1280, height: 820 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto(url);
await page.waitForSelector('a.folder');

// search via quick switcher
await page.keyboard.press('Meta+k');
await page.fill('input[aria-label="Search"]', 'pumpkin pie');
await page.waitForSelector('li[role=option] button.sel');
await page.keyboard.press('Enter');
await page.waitForSelector('.prose h1:has-text("Pumpkin Pie")');

// scaling ×2 changes "½ cup sugar" to "1 cup sugar"
await page.click('button[aria-label="More servings"]');
await page.click('button[aria-label="More servings"]');
assert.ok(await page.locator('.prose li', { hasText: /^1 cup sugar$/ }).count() > 0, 'scaled sugar line');
await page.screenshot({ path: `${out}/scaled.png` });

// cook mode
await page.click('text=Cook mode');
await page.waitForSelector('.cook li');
await page.locator('.cook input[type=checkbox]').first().check();
await page.screenshot({ path: `${out}/cook.png` });
await page.click('.cook >> text=Done');

// edit + autosave writes to disk
await page.click('button:has-text("Edit")');
await page.waitForSelector('.cm-content');
await page.click('.cm-content');
await page.keyboard.press('Meta+End');
await page.keyboard.type('\nAdded from the browser test.\n');
let disk = '';
for (let i = 0; i < 25 && !disk.includes('Added from the browser test.'); i++) {
  await page.waitForTimeout(200);
  disk = fs.readFileSync(`${vault}/Pies/Pumpkin Pie.md`, 'utf8');
}
assert.ok(disk.includes('Added from the browser test.'), 'autosaved to disk');
await page.screenshot({ path: `${out}/split.png` });

// external edit while editing -> conflict banner
await page.keyboard.type('more');
fs.writeFileSync(`${vault}/Pies/Pumpkin Pie.md`, disk + '\nChanged in another editor.\n');
await page.waitForSelector('text=changed somewhere else', { timeout: 8000 });
await page.click('button:has-text("Use theirs")');
await page.click('button:has-text("Read")');
await page.waitForSelector('text=Changed in another editor.');

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('flows ok');
