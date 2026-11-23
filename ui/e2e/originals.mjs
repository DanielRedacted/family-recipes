// Originals: node e2e/originals.mjs <admin-url> <vault> <out>
// A file a recipe points at with `original:` is reached from that recipe only; one nobody points at is
// listed and can be converted where it is (it then moves under attachments/originals).
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const at = (p) => `${vault}/${p}`;
fs.mkdirSync(at('attachments/originals'), { recursive: true });
fs.mkdirSync(at('Pies'), { recursive: true });
fs.writeFileSync(at('attachments/originals/Linked Pie.docx'), 'not really Word');
fs.writeFileSync(at('Pies/Linked Pie.md'), '---\noriginal: "/attachments/originals/Linked Pie.docx"\n---\n\n# Linked Pie\n\n## Ingredients\n\n- 1 crust\n\n## Directions\n\n1. Bake.\n');
fs.writeFileSync(at('Pies/Loose Tart.txt'), 'Loose Tart\n\nIngredients\n2 cups flour\n1 cup butter\n\nDirections\nMix and bake for 30 minutes.\n');

const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1280, height: 860 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
const go = async (hash) => { await page.goto(new URL('/' + hash, url).href); };
await page.goto(url);
await page.request.put(new URL('/api/settings', url).href, { headers: { 'X-Recipes': '1', Origin: new URL(url).origin }, data: { setup_done: true } });
await page.reload();
await page.waitForSelector('a.folder');

// the linked .docx isn't listed or searchable; the recipe's menu opens it, and it offers Download
await go('#/f/Pies');
await page.waitForSelector('.recipe:has-text("Linked Pie")');
await page.keyboard.press('Meta+k');
await page.fill('input[aria-label="Search"]', 'Linked Pie');
await page.waitForTimeout(500);
assert.equal(await page.locator('li[role=option]:has-text("docx")').count(), 0, 'linked original not in search');
await page.keyboard.press('Escape');
await go('#/n/Pies/Linked%20Pie.md');
await page.waitForSelector('.prose h1:has-text("Linked Pie")');
await page.click('button[aria-label="More"]');
await page.click('.menu button:has-text("Original file")');
await page.waitForSelector('a.btn[download]:has-text("Download")');
await page.waitForSelector('text=The original of');

// a loose .txt shows its text and converts where it is; on save it moves under the recipe
await go('#/n/Pies/Loose%20Tart.txt');
await page.waitForSelector('pre:has-text("2 cups flour")');
await page.click('button:has-text("Convert to an editable recipe")');
await page.waitForSelector('.rev input.title', { timeout: 30000 });
await page.click('button:has-text("Save recipe")');
await page.waitForSelector('.prose h1', { timeout: 20000 });
await page.waitForTimeout(800);
assert.ok(!fs.existsSync(at('Pies/Loose Tart.txt')), 'the .txt left the folder');
assert.ok(fs.existsSync(at('attachments/originals/Loose Tart.txt')), 'the .txt is kept as the original');
const made = fs.readdirSync(at('Pies')).find((n) => n.endsWith('.md') && n !== 'Linked Pie.md' && fs.readFileSync(at(`Pies/${n}`), 'utf8').includes('Loose Tart.txt'));
assert.ok(made && fs.readFileSync(at(`Pies/${made}`), 'utf8').includes('original: "/attachments/originals/Loose Tart.txt"'), 'the recipe points at it');
await page.screenshot({ path: `${out}/originals.png` });

assert.deepEqual(errors, []);
await browser.close();
console.log('originals ok');
