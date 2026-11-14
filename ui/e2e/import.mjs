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

// 1. web page with image picker
await imp();
await page.selectOption('label.dest select', 'Cookies');
await page.fill('input[aria-label="Recipe web address"]', 'https://www.allrecipes.com/recipe/10813/best-chocolate-chip-cookies/');
await page.click('button:has-text("Get recipe")');
await page.waitForSelector('.rev input.title', { timeout: 30000 });
await page.waitForTimeout(2500); // thumbnails load, small ones hide themselves
await page.screenshot({ path: `${out}/import-review.png` });
const figs = page.locator('figure');
const n = await figs.count();
if (n > 1) await figs.nth(1).locator('input[type=checkbox]').check();
await page.click('button:has-text("Save recipe")');
await page.waitForSelector('.prose h1:has-text("Best Chocolate Chip Cookies")', { timeout: 30000 });
const saved = fs.readFileSync(`${vault}/Cookies/Best Chocolate Chip Cookies.md`, 'utf8');
assert.ok(/!\[Best Chocolate Chip Cookies\]\(\/attachments\/best-chocolate-chip-cookies\.(jpg|webp|png)\)/.test(saved), 'cover image line');
assert.ok(fs.readdirSync(`${vault}/attachments`).some((f) => f.startsWith('best-chocolate-chip-cookies')), 'cover saved');
await page.screenshot({ path: `${out}/import-saved.png` });

// 2. PDF kept as view-only
await imp();
await page.setInputFiles('input[type=file]', `${SRC}/PDFs/Einkorn Pizza Dough Recipe _ GrillGirl.pdf`);
await page.waitForSelector('text=Keep as a PDF');
await page.fill('.choice input >> nth=0', 'Einkorn Pizza Dough (GrillGirl)');
await page.selectOption('.choice select', 'Pizza');
await page.click('.choice button:has-text("Save")');
await page.waitForSelector('.pages canvas', { timeout: 20000 });
assert.ok(fs.existsSync(`${vault}/attachments/originals/Einkorn Pizza Dough Recipe _ GrillGirl.pdf`), 'original pdf kept');
const note = fs.readFileSync(`${vault}/Pizza/Einkorn Pizza Dough (GrillGirl).md`, 'utf8');
assert.ok(note.includes('original: "/attachments/originals/Einkorn Pizza Dough Recipe _ GrillGirl.pdf"'));
await page.screenshot({ path: `${out}/pdf-keep.png` });

// 3. convert that PDF later from the viewer
await page.click('button:has-text("Convert to an editable recipe")');
await page.waitForSelector('.rev input.title', { timeout: 30000 });
await page.screenshot({ path: `${out}/pdf-convert-review.png` });
await page.click('button:has-text("Cancel")');

// 4. Word document
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

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('import flows ok');
