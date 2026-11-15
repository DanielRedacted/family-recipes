// Ingredient catalogue and editor suggestions: node e2e/ingredients.mjs <admin-url> <vault> <out>
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1280, height: 860 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto(url);
await page.waitForSelector('a.folder');

// the catalogue page groups spellings
await page.click('button:has-text("Ingredients")');
await page.fill('input[aria-label="Find an ingredient"]', 'egg');
await page.click('.head:has-text("eggs") >> nth=0');
await page.waitForSelector('.more li a');
assert.ok(Number((await page.locator('.list > li.open .head .muted').textContent()).match(/\d+/)[0]) >= 40, 'eggs used widely');
await page.screenshot({ path: `${out}/ingredients.png` });

// ⌘K finds ingredients too
await page.keyboard.press('Meta+k');
await page.fill('input[aria-label="Search"]', 'baking soda');
await page.waitForSelector('li[role=option]:has-text("ingredient")');
await page.keyboard.press('Escape');

// typing an ingredient line suggests units, then ingredient names
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md?mode=edit', url).href);
await page.waitForSelector('.cm-content');
await page.locator('.cm-line', { hasText: '15 oz can pumpkin' }).click();
await page.keyboard.press('End');
await page.keyboard.type('\n2 tb');
await page.waitForSelector('.cm-tooltip-autocomplete li:has-text("tbsp")');
await page.keyboard.press('Enter');
await page.keyboard.type(' brown su');
await page.waitForSelector('.cm-tooltip-autocomplete li:has-text("brown sugar")');
await page.screenshot({ path: `${out}/ingredient-suggest.png` });
await page.keyboard.press('Enter');
await page.click('button:has-text("Save")');
await page.waitForTimeout(800);
assert.ok(/^- 2 tbsp brown sugar$/m.test(fs.readFileSync(`${vault}/Pies/Pumpkin Pie.md`, 'utf8')), 'completed ingredient line saved');

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('ingredients ok');
