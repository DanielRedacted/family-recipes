// Import flows: node e2e/import.mjs <admin-url> <vault> <out>
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const SRC = '/Users/daniel/Desktop/recipe stuff/cookbook/recipes';
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1280, height: 860 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto(url);
await page.waitForSelector('a.folder');
const imp = () => page.goto(new URL('/#/import', url).href);

// 1. two web addresses at once: a schema.org page and a WP Recipe Maker site
await imp();
await page.selectOption('label.dest select', 'Cookies');
await page.fill('textarea[aria-label="Recipe web addresses"]',
  'https://www.allrecipes.com/recipe/10813/best-chocolate-chip-cookies/\nhttps://www.recipetineats.com/chicken-chasseur/');
await page.click('button:has-text("Get recipe")');
await page.waitForSelector('.rev input.title', { timeout: 45000 });
await page.waitForTimeout(2500); // thumbnails load, small ones hide themselves
await page.screenshot({ path: `${out}/import-review.png` });
const figs = page.locator('figure');
if ((await figs.count()) > 1) await figs.nth(1).locator('input[type=checkbox]').check();
await page.click('details summary:has-text("Import details")');
assert.ok(await page.locator('.log li').count() >= 4, 'import log shown');
await page.click('button:has-text("Save recipe")');
// the second item is reviewed next
await page.waitForFunction(() => document.querySelector('.rev input.title')?.value?.includes('Chasseur'), null, { timeout: 45000 });
assert.ok(await page.locator('.log li:has-text("wprm-rest")').count(), 'WP Recipe Maker REST step logged');
await page.screenshot({ path: `${out}/import-review-wprm.png` });
await page.click('button:has-text("Save recipe")');
await page.waitForSelector('.prose h1', { timeout: 30000 });
const cookies = fs.readFileSync(`${vault}/Cookies/Best Chocolate Chip Cookies.md`, 'utf8');
assert.ok(/!\[Best Chocolate Chip Cookies\]\(\/attachments\/best-chocolate-chip-cookies\.(jpg|webp|png)\)/.test(cookies), 'cover image line');
assert.ok(/nutrition:\n {2}calories: /.test(cookies), 'nutrition kept');
const chasseur = fs.readFileSync(`${vault}/Cookies/JB's Chicken Chasseur.md`, 'utf8');
assert.ok(chasseur.includes('### Sauce Chasseur'), 'WPRM ingredient groups');
assert.ok(chasseur.includes('vitamin_a:'), 'WPRM nutrition');
assert.ok(await page.locator('.nutrition').count(), 'nutrition table in reader');
await page.screenshot({ path: `${out}/import-saved.png` });

// 2. PDF kept as view-only
await imp();
await page.selectOption('label.dest select', 'Pizza');
await page.setInputFiles('input[type=file]', `${SRC}/PDFs/Einkorn Pizza Dough Recipe _ GrillGirl.pdf`);
await page.waitForSelector('text=Keep as a PDF');
await page.fill('.choice input[aria-label="Title"]', 'Einkorn Pizza Dough (GrillGirl)');
await page.click('.choice button:has-text("Keep")');
await page.waitForSelector('.pages canvas', { timeout: 20000 });
assert.ok(fs.existsSync(`${vault}/attachments/originals/Einkorn Pizza Dough Recipe _ GrillGirl.pdf`), 'original pdf kept');
const note = fs.readFileSync(`${vault}/Pizza/Einkorn Pizza Dough (GrillGirl).md`, 'utf8');
assert.ok(note.includes('original: "/attachments/originals/Einkorn Pizza Dough Recipe _ GrillGirl.pdf"'));
await page.screenshot({ path: `${out}/pdf-keep.png` });

// 3. convert that PDF later from the viewer
await page.click('button:has-text("Convert to an editable recipe")');
await page.waitForSelector('.rev input.title', { timeout: 30000 });
await page.screenshot({ path: `${out}/pdf-convert-review.png` });
await page.click('button:has-text("Discard")');

// 4. Word document and a pasted recipe queued together
await imp();
await page.selectOption('label.dest select', 'Cookies');
await page.setInputFiles('input[type=file]', `${SRC}/images/Cookies - Macaroons.docx`);
await page.waitForSelector('.rev input.title', { timeout: 30000 });
await page.screenshot({ path: `${out}/docx-review.png` });
await page.fill('.rev input.title', 'Macaroons');
await page.click('button:has-text("Save recipe")');
await page.waitForSelector('.prose h1', { timeout: 20000 });
const mac = fs.readFileSync(`${vault}/Cookies/Macaroons.md`, 'utf8');
assert.ok(mac.includes('original: "/attachments/originals/Cookies - Macaroons.docx"'), 'docx original kept');
await page.screenshot({ path: `${out}/docx-saved.png` });

// 5. a failing address shows its error and log, and can be removed
await imp();
await page.fill('textarea[aria-label="Recipe web addresses"]', 'https://www.recipetineats.com/this-page-does-not-exist-xyz/');
await page.click('button:has-text("Get recipe")');
await page.waitForSelector('.qi.failed', { timeout: 30000 });
assert.ok(await page.locator('.qi.failed .log li').count(), 'failure log shown');
await page.screenshot({ path: `${out}/import-failed.png` });
await page.click('.qi.failed button[aria-label="Remove"]');
assert.equal(await page.locator('.qi').count(), 0);

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('import flows ok');
