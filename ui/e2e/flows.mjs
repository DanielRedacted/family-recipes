// End-to-end flows against a running server: node e2e/flows.mjs <admin-url> <vault-dir> <outdir>
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, vault, out] = process.argv.slice(2);
const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 1280, height: 820 } });
const page = await context.newPage();
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

// the navigation column: no "+", recipe icons, follows the open page, one window tall
assert.equal(await page.locator('aside button[aria-label="Add"]').count(), 0, 'no + in the nav column');
await page.goto(new URL('/#/n/Soups%20%26%20Chili/Gumbo.md', url).href);
await page.waitForSelector('aside [aria-current="page"]:has-text("Gumbo")');
assert.ok(await page.locator('aside [aria-current="page"] .emoji').count(), 'recipe icon in the tree');
assert.equal(await page.locator('aside .row-item.folder:has-text("Pies")').getAttribute('aria-expanded'), 'false', 'folders off the path fold up');
await page.click('aside .row-item.folder:has-text("Breads") >> nth=0 >> .chev');
await page.waitForTimeout(300);
assert.equal(await page.locator('aside .row-item.folder:has-text("Breads") >> nth=0').getAttribute('aria-expanded'), 'true', 'a folder opened by hand stays open');
assert.deepEqual((await page.locator('.crumbs > *').allTextContents()).filter((t) => t.trim()), ['All recipes', '›', 'Soups & Chili', '›', 'Gumbo']);
const [asideH, winH] = await page.evaluate(() => [document.querySelector('aside').getBoundingClientRect().height, innerHeight]);
assert.equal(asideH, winH, 'nav column is one window tall');
// folder cards show only the count, in the corner
await page.goto(new URL('/#/', url).href);
assert.match((await page.locator('a.folder:has-text("Pies") .count').textContent()).trim(), /^\d+$/);
assert.equal(await page.locator('a.folder:has-text("items")').count(), 0);

// bookmarks: from the ⋯ menu, stored as a property, shown on the home page and in the tree
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
await page.waitForSelector('.prose h1');
await page.click('header button[aria-label="More"]');
assert.equal(await page.locator('.menu button:has-text("Download")').count(), 0, 'no Download');
await menuItem('Bookmark for everyone');
await page.waitForTimeout(800);
assert.match(fs.readFileSync(pie, 'utf8'), /^---\n(?:.*\n)*favorite: true\n(?:.*\n)*---\n/, 'favorite saved as a property (block style)');
await page.goto(new URL('/#/', url).href);
await page.waitForSelector('.sect:has(h2:has-text("Bookmarks")) .recipe:has-text("Pumpkin Pie")');
// a home section folds, and stays folded on this device
const bookmarksBtn = page.locator('.sect h2 button:has-text("Bookmarks")');
await bookmarksBtn.click();
assert.equal(await bookmarksBtn.getAttribute('aria-expanded'), 'false');
await page.reload();
await page.waitForSelector('a.folder');
assert.equal(await page.locator('.sect:has(h2:has-text("Bookmarks")) .recipe:has-text("Pumpkin Pie")').isVisible(), false, 'still folded after a reload');
await page.locator('.sect h2 button:has-text("Bookmarks")').click();
await page.waitForSelector('.sect:has(h2:has-text("Bookmarks")) .recipe:has-text("Pumpkin Pie")');
await page.waitForSelector('aside .row-item:has-text("Bookmarks")');
await page.screenshot({ path: `${out}/bookmarks.png` });

// renamed recipes are still found by their old names (aliases:)
await page.click('aside button[aria-label="Search"]');
await page.fill('input[aria-label="Search"]', 'Tigernut');
await page.waitForSelector('dialog[aria-label=Search] >> text=Grain and Sugar Free Pumpkin Muffins');
await page.keyboard.press('Escape');

// Settings has its own navigation column; recent changes and trash are pages in it
await page.goto(new URL('/#/settings', url).href);
await page.waitForSelector('aside nav[aria-label="Settings"]');
await page.click('aside a:has-text("Recent changes")');
await page.waitForSelector('.recent a:has-text("Pumpkin Pie")');
await page.click('aside a:has-text("Trash")');
await page.waitForSelector('h1:has-text("Trash")');
assert.equal(await page.locator('text=Download everything').count(), 0);
await page.click('aside a:has-text("Recipes")');
await page.waitForSelector('aside .tree');
assert.equal((await page.request.get(new URL('/api/download', url).href)).status() >= 400 || !(await page.request.get(new URL('/api/download', url).href)).headers()['content-type']?.includes('zip'), true, 'download route gone');

// ingredient tables with alternative portions: one portion at a time, its multiplier on the chip
const ingText = () => page.locator('.prose li').allTextContents();
await page.goto(new URL('/#/n/Pies/Pie%20Crust%20(Grandma).md', url).href);
await page.waitForSelector('.portions');
assert.deepEqual((await page.locator('.portions .chip').allTextContents()).map((t) => t.replace(/\s+/g, ' ').trim()), ['Single ×1', 'Double ×2']);
assert.ok((await ingText()).includes('1 ½ cups AP flour'), 'Single amounts shown');
assert.equal(await page.locator('.prose table').count(), 0, 'the table is shown as a list');
await page.click('.portions .chip:has-text("Double")');
assert.ok((await ingText()).includes('3 cups AP flour'), 'Double amounts shown');
assert.ok((await ingText()).includes('1½ tsp Salt'), "the family's Double amount, not 2 × 1 tsp");
assert.equal((await page.locator('.scaler .qty').textContent()).trim(), '×2');
await page.screenshot({ path: `${out}/portions.png` });
await page.click('button:has-text("Cook mode")');
await page.waitForSelector('.cook li:has-text("3 cups AP flour")');
await page.click('.cook button:has-text("Done")');
await page.goto(new URL('/#/n/Eggs/Quiche%20Lorraine.md', url).href);
await page.waitForSelector('.portions .chip:has-text("10” Quiche")');
assert.match(await page.locator('.portions .chip:has-text("10” Quiche")').textContent(), /×1½/);
// versions rather than sizes: no multiplier, and an ingredient only one version uses
await page.goto(new URL('/#/n/Desserts/Ice%20Cream.md', url).href);
await page.waitForSelector('.portions');
assert.ok(!(await ingText()).some((t) => /Unsweetened baking chocolate/.test(t)), 'Vanilla has no chocolate');
await page.click('.portions .chip:has-text("Chocolate")');
assert.ok((await ingText()).includes('2 oz Unsweetened baking chocolate'));
assert.doesNotMatch(await page.locator('.portions').textContent(), /×/);
// a yield in dozens
await page.goto(new URL('/#/n/Cookies/Chocolate%20Boiled%20Oatmeal%20Cookies.md', url).href);
await page.waitForSelector('.scaler .qty');
assert.equal((await page.locator('.scaler .qty').textContent()).trim(), '24 (2 dozen)');
await page.click('button[aria-label="More servings"]');
assert.equal((await page.locator('.scaler .qty').textContent()).trim(), '30 (2½ dozen)');
assert.ok((await ingText()).some((t) => t.startsWith('2½ cups')), 'every ingredient scaled: ' + (await ingText())[0]);
// every set of ingredients scales, not only the first (Pizza Sauce comes after the dough's steps)
await page.goto(new URL('/#/n/Breads/Pizza%20Dough%20%26%20Sauce.md', url).href);
await page.waitForSelector('.portions');
await page.click('.portions .chip:has-text("Large")');
assert.ok((await ingText()).includes('12 oz Liquid (water or beer)'));
assert.ok((await ingText()).some((t) => /^11 oz canned tomato paste/.test(t)), 'the sauce scales too: ' + (await ingText()).filter((t) => /tomato/.test(t)));
// no table rows joined with " · " in the ingredient catalogue
await page.goto(new URL('/#/ingredients', url).href);
await page.waitForSelector('.ing, table, li');
assert.equal(await page.locator('text= · ').count(), 0);
// the bundled tables are already in the editor's layout: opening, editing elsewhere and saving keeps them
const crust = `${vault}/Pies/Pie Crust (Grandma).md`;
const crustBefore = fs.readFileSync(crust, 'utf8');
await page.goto(new URL('/#/n/Pies/Pie%20Crust%20(Grandma).md?mode=edit', url).href);
await page.waitForSelector('.tbl-table');
await page.locator('.cm-line').last().click(); // not the title: other recipes link to it by name
await page.keyboard.press('End');
await page.keyboard.type('!');
await page.click('button:has-text("Save")');
await page.waitForTimeout(800);
const crustAfter = fs.readFileSync(crust, 'utf8');
assert.equal(crustAfter.split('\n').filter((l) => l.startsWith('|')).join('\n'), crustBefore.split('\n').filter((l) => l.startsWith('|')).join('\n'), 'table text unchanged by the editor');
fs.writeFileSync(crust, crustBefore);

// links work on a plain click; Back (and ⌘[) returns
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
await page.waitForSelector('.prose a.wikilink');
await page.click('.prose a.wikilink');
await page.waitForFunction(() => location.hash.includes('Pie%20Crust'));
await page.click('button[aria-label="Back"]:visible');
await page.waitForFunction(() => location.hash.includes('Pumpkin'));
await page.click('.prose a.wikilink');
await page.waitForFunction(() => location.hash.includes('Pie%20Crust'));
await page.keyboard.press('Meta+BracketLeft');
await page.waitForFunction(() => location.hash.includes('Pumpkin'));
const fresh = await context.newPage();
await fresh.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
await fresh.waitForSelector('.prose h1');
assert.ok(await fresh.locator('button[aria-label="Back"]:visible').isDisabled(), 'nothing to go back to on the first page');
await fresh.close();
// a web page link is marked ↗ and opens outside the app; the page stays where it is
await page.goto(new URL('/#/n/Cookies/Chocolate%20Boiled%20Oatmeal%20Cookies.md', url).href);
const ext = page.locator('.prose a.ext').first();
await ext.waitFor();
assert.equal(await ext.evaluate((a) => getComputedStyle(a, '::after').content), '"↗"');
assert.equal(await page.locator('.prose a.wikilink.ext, .prose a.internal.ext').count(), 0);
const [popup] = await Promise.all([page.waitForEvent('popup'), ext.click()]);
assert.match(popup.url(), /food\.com/);
await popup.close();
assert.ok((await page.evaluate(() => location.hash)).includes('Oatmeal'), 'still on the recipe');
// a missing property shows its label with Add behind More in the recipe's card; filling it in saves it
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
assert.equal(await page.locator('button[aria-label="Add Prep"]').count(), 0, 'add pills wait behind More');
await page.click('.recipebar button.toggle');
await page.click('button[aria-label="Add Prep"]');
await page.keyboard.type('20 min');
await page.keyboard.press('Enter');
await page.waitForTimeout(800);
assert.match(fs.readFileSync(pie, 'utf8'), /^---\n(?:.*\n)*prep_time: 20 min\n/, 'prep time saved');
await page.waitForSelector('.props .chip:has-text("20 min")');
assert.equal(await page.locator('button[aria-label="Add Prep"]').count(), 0);
await page.screenshot({ path: `${out}/props.png` });
// a recipe sits indented under its folder in the navigation column
const icX = async (sel) => (await page.locator(sel).first().locator('.ic').boundingBox()).x;
const folderX = await icX('aside .row-item.folder:has-text("Pies")');
const fileX = await icX('aside .row-item:has-text("Pumpkin Pie"):not(.folder)');
assert.ok(fileX > folderX + 8, `file icon ${fileX} indented from folder icon ${folderX}`);

// units: one spelling in the files; Settings → General offers US and Metric; US tidies scaled amounts
const units = async (u) => {
  await page.goto(new URL('/#/settings/general', url).href);
  await page.waitForSelector('select[aria-label="Units"]');
  assert.deepEqual(await page.locator('select[aria-label="Units"] option').allTextContents(), ['US', 'Metric']);
  await page.selectOption('select[aria-label="Units"]', u);
  await page.goto(new URL('/#/n/Candy/Caramel%20and%20Sauce.md', url).href);
  await page.waitForSelector('.prose li');
};
await units('us');
assert.ok((await ingText()).includes('16 tbsp Butter'), 'as written at ×1');
await page.click('button[aria-label="Fewer"]');
assert.ok((await ingText()).includes('½ cup Butter'), '16 tbsp × ½ reads ½ cup: ' + (await ingText()).slice(0, 3));
await units('metric');
assert.ok((await ingText()).some((t) => /^\d+ ml Butter$/.test(t)), 'metric converts');
await units('us');

// tabs: each keeps its own page, Back steps and scroll; switching leaves the folder column alone
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
await page.waitForSelector('.pane:not([inert]) .prose h1');
const tabs = () => page.locator('.tabs [role="tab"]');
const shown = '.pane:not([inert])';
for (let n = await tabs().count(); n > 1; n--) await page.click('.tabs [role="tab"]:not([aria-selected="true"]) .x');
await page.click('aside .row-item:has-text("Pie Crust"):not(.folder)', { modifiers: ['Meta'] });
await page.waitForFunction(() => location.hash.includes('Pie%20Crust'));
assert.equal(await tabs().count(), 2, 'a new tab');
assert.match(await page.locator('.tabs [aria-selected="true"]').innerText(), /Pie Crust/);
assert.ok(await page.locator(`${shown} button[aria-label="Back"]`).isDisabled(), 'a new tab has nothing to go back to');
await page.goto(new URL('/#/n/Candy/Caramel%20and%20Sauce.md', url).href);
await page.waitForSelector(`${shown} .prose h1:has-text("Caramel")`);
await page.click(`${shown} button[aria-label="Back"]`);
await page.waitForFunction(() => location.hash.includes('Pie%20Crust'));
assert.match(await page.locator(`${shown} .prose h1`).innerText(), /Pie Crust/, 'Back steps within the tab');
// the first tab: still the pie, scaled and scrolled as left
await page.click('.tabs [role="tab"] >> nth=0');
await page.waitForFunction(() => location.hash.includes('Pumpkin'));
await page.click(`${shown} button[aria-label="More servings"]`);
const qty = await page.locator(`${shown} .qty`).innerText();
const y = await page.locator(shown).evaluate((el) => { el.scrollTop = 120; return el.scrollTop; });
assert.ok(y > 0, 'the page scrolls');
const expandedRows = async () => (await page.locator('aside [aria-expanded="true"]').allInnerTexts()).join('|');
const folds = await expandedRows();
await page.click('.tabs [role="tab"] >> nth=1');
await page.waitForFunction(() => location.hash.includes('Pie%20Crust'));
assert.equal(await expandedRows(), folds, 'switching tabs leaves the folders as they are');
await page.click('.tabs [role="tab"] >> nth=0');
assert.equal(await page.locator(`${shown} .qty`).innerText(), qty, 'the scaler held');
assert.equal(await page.locator(shown).evaluate((el) => el.scrollTop), y, 'the scroll held');
assert.equal(await page.locator('aside .row-item.active .name').last().innerText(), 'Pumpkin Pie', 'the column highlights the tab’s page');
// the tabs come back after a reload
await page.reload();
await page.waitForSelector(`${shown} .prose h1`);
assert.equal(await tabs().count(), 2, 'tabs kept');
// a new tab always goes on the right, whichever tab is open
await page.click('.tabs [role="tab"] >> nth=0');
await page.click('.tabs button[aria-label="New tab"]');
assert.equal(await page.locator('.tabs [role="tab"]').nth(2).getAttribute('aria-selected'), 'true', 'new tab at the end');
await page.click('.tabs [role="tab"] >> nth=2 >> .x');
// closing the last tab leaves a home tab
await page.click('.tabs [role="tab"] >> nth=1 >> .x');
await page.click('.tabs [role="tab"] .x');
assert.equal(await tabs().count(), 1);
assert.match(await tabs().first().innerText(), /Recipes/);
await page.waitForSelector(`${shown} a.folder`);
// an unsaved edit survives switching tabs; closing its tab asks
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md?mode=edit', url).href);
await page.waitForSelector(`${shown} .cm-content`);
await page.click(`${shown} .cm-content`);
await page.keyboard.press('Meta+End');
await page.keyboard.type('\nkept in its tab');
await page.click('.tabs button[aria-label="New tab"]');
await page.waitForSelector(`${shown} a.folder`);
assert.equal(await page.locator('text=Save your changes?').count(), 0, 'no question on switching');
await page.click('.tabs [role="tab"] >> nth=0');
assert.ok((await page.locator(`${shown} .cm-content`).innerText()).includes('kept in its tab'), 'the edit held');
await page.click('.tabs [role="tab"] >> nth=0 >> .x');
await page.waitForSelector('text=Save your changes?');
await page.click('button:has-text("Don’t save")');
await page.waitForFunction(() => document.querySelectorAll('.tabs [role="tab"]').length === 1);
assert.ok(!fs.readFileSync(pie, 'utf8').includes('kept in its tab'));
// a recipe renamed while open in another tab: that tab follows it
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
await page.waitForSelector(`${shown} .prose h1`);
await page.click('.tabs button[aria-label="New tab"]');
await page.goto(new URL('/#/f/Pies', url).href);
const renamePie = async (name) => {
  await page.click(`${shown} .recipe:has-text("Pumpkin Pie")`, { button: 'right' });
  await menuItem('Rename');
  await page.fill('.modal input', name);
  await page.click('.modal button:has-text("Rename")');
  await page.waitForSelector('.toast');
};
await renamePie('Pumpkin Pie 2');
await page.click('.tabs [role="tab"] >> nth=0');
await page.waitForFunction(() => location.hash.includes('Pumpkin%20Pie%202'));
await page.waitForSelector(`${shown} .prose h1:has-text("Pumpkin Pie")`);
// and renamed back from its own menu, in the tab showing it
await page.click(`${shown} button[aria-label="More"]`);
await menuItem('Rename');
await page.fill('.modal input', 'Pumpkin Pie');
await page.click('.modal button:has-text("Rename")');
await page.waitForFunction(() => /Pumpkin%20Pie\.md$/.test(location.hash));
assert.ok(fs.existsSync(pie), 'renamed back');
await page.click('.tabs [role="tab"] >> nth=1 >> .x');
// a phone has no strip; its Back button and the browser's Back are the same steps
await page.setViewportSize({ width: 390, height: 800 });
assert.equal(await page.locator('.tabs').isVisible(), false, 'no tab strip on a phone');
await page.goto(new URL('/#/n/Pies/Pumpkin%20Pie.md', url).href);
await page.waitForSelector(`${shown} .prose a.wikilink`);
await page.click(`${shown} .prose a.wikilink`);
await page.waitForFunction(() => location.hash.includes('Pie%20Crust'));
await page.click(`${shown} button[aria-label="Back"]:visible`);
await page.waitForFunction(() => location.hash.includes('Pumpkin'));
await page.goForward();
await page.waitForFunction(() => location.hash.includes('Pie%20Crust'));
await page.goBack();
await page.waitForFunction(() => location.hash.includes('Pumpkin'));
await page.setViewportSize({ width: 1280, height: 820 });

await browser.close();
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log('flows ok');
