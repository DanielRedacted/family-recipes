// End-to-end flows against a running server: node e2e/flows.mjs <admin-url> <vault-dir> <outdir>
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1280, height: 820 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
const pie = `${vault}/Pies/Pumpkin Pie.md`;
const more = () => page.click('button[aria-label="More"]');
const menuItem = (label) => page.click(`.menu button:has-text("${label}")`);
await page.goto(url);
await page.waitForSelector('a.folder');

// folder cards carry an emoji; no zip link, no "Recently changed" on the home page
assert.ok(await page.locator('a.folder:has-text("Pies") .emoji').count(), 'folder emoji');
assert.equal(await page.locator('text=Recently changed').count(), 0);

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

// Edit lives in the ⋯ menu; typing does not write to disk, Save does
const before = fs.readFileSync(pie, 'utf8');
await more();
await menuItem('Edit');
await page.waitForSelector('.cm-content');
await page.click('.cm-content');
await page.keyboard.press('Meta+End');
await page.keyboard.type('\nAdded from the browser test. See [[Pie Cr');
await page.waitForSelector('.cm-tooltip-autocomplete li:has-text("Pie Crust")');
await page.keyboard.press('Enter');
await page.waitForTimeout(1500);
assert.equal(fs.readFileSync(pie, 'utf8'), before, 'nothing written before Save');
assert.ok(await page.locator('text=Unsaved changes').count());
await page.screenshot({ path: `${out}/editing.png` });
await page.click('button:has-text("Save")');
let disk = '';
for (let i = 0; i < 25 && !disk.includes('Added from the browser test.'); i++) {
  await page.waitForTimeout(200);
  disk = fs.readFileSync(pie, 'utf8');
}
assert.ok(disk.includes('See [[9” Pie Crust]]'), 'saved with the completed link');

// leaving with unsaved changes asks; "Don't save" discards
await page.click('.cm-content');
await page.keyboard.press('Meta+End');
await page.keyboard.type('\nthrow this away');
await page.click('aside a.logo');
await page.waitForSelector('text=Save your changes?');
await page.click('button:has-text("Don’t save")');
await page.waitForSelector('a.folder');
assert.ok(!fs.readFileSync(pie, 'utf8').includes('throw this away'));

// the linked recipe lists where it is mentioned
await page.goto(new URL('/#/n/Pies/Pie%20Crust.md', url).href);
await page.waitForSelector('.mentions a:has-text("Pumpkin Pie")');

// external edit while editing -> conflict banner
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
await page.waitForSelector('.prose h1');
await page.keyboard.press('Meta+e');
await page.click('.cm-content');
await page.keyboard.type('more');
fs.writeFileSync(pie, disk + '\nChanged in another editor.\n');
await page.waitForSelector('text=changed somewhere else', { timeout: 8000 });
await page.click('button:has-text("Use theirs")');
await page.click('button:has-text("Done")');
await page.waitForSelector('text=Changed in another editor.');

// icon & colour from the card's menu, rename from right-click
await page.goto(new URL('/#/f/Pies', url).href);
await page.waitForSelector('.recipe');
await page.click('.recipe:has-text("Pumpkin Pie")', { button: 'right' });
await menuItem('Icon & colour');
await page.fill('input[aria-label="Emoji"]', '🎃');
await page.click('.sw[aria-label="orange"]');
await page.click('form.modal button:has-text("Save")');
await page.waitForTimeout(800);
const fm = fs.readFileSync(pie, 'utf8');
assert.ok(fm.startsWith('---\n') && fm.includes('emoji: 🎃') && fm.includes('color: orange'), 'icon saved in frontmatter');
await page.goto(new URL('/#/', url).href);
await page.click('a.folder:has-text("Pies")', { button: 'right' });
await menuItem('Icon & colour');
await page.click('.sw[aria-label="purple"]');
await page.click('form.modal button:has-text("Save")');
await page.waitForTimeout(800);
assert.ok(fs.readFileSync(`${vault}/.folders.yaml`, 'utf8').includes('Pies:\n  color: purple'), 'folder colour saved');
await page.goto(new URL('/#/f/Pies', url).href);
await page.click('.recipe:has-text("Pie Crust")', { button: 'right' });
await menuItem('Rename');
await page.fill('.modal input', 'Pie Crust (Grandma)');
await page.click('.modal button:has-text("Rename")');
await page.waitForSelector('.recipe:has-text("Pie Crust")');
assert.ok(fs.existsSync(`${vault}/Pies/Pie Crust (Grandma).md`));
await page.screenshot({ path: `${out}/folder-icons.png` });

// one "+" button with every way to add
await page.click('.toolbar button[aria-label="Add"]');
for (const l of ['Blank recipe', 'From a web address', 'From a file', 'Paste recipe text', 'Upload files here', 'New folder']) {
  assert.ok(await page.locator(`.menu button:has-text("${l}")`).count(), l);
}
await page.screenshot({ path: `${out}/plus-menu.png` });
await menuItem('From a web address');
await page.waitForSelector('textarea[aria-label="Recipe web addresses"]:focus');

// recent changes and trash live in Settings
await page.goto(new URL('/#/settings', url).href);
await page.waitForSelector('h2:has-text("Recent changes")');
assert.ok(await page.locator('.recent a:has-text("Pumpkin Pie")').count());
assert.ok(await page.locator('a:has-text("Open trash")').count());

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('flows ok');
