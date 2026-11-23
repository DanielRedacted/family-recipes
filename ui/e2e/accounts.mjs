// Sign-in, the write log, the whiteboard, favorites, bookmarks and people pages: node e2e/accounts.mjs <admin-url> <vault-dir> <outdir>
// A second browser (an iPhone, without the desktop app's cookie) plays the family member. Run on a copy.
import { chromium, devices } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
import { parse as parseYaml } from 'yaml';
const [url, vault, out] = process.argv.slice(2);
const origin = new URL(url).origin;
const pie = `${vault}/Pies/Pumpkin Pie.md`;
const browser = await chromium.launch();
const errors = [];
const watch = (p, who) => p.on('pageerror', (e) => errors.push(`${who}: ${e.message}`));
const call = (page, method, path, body) => page.evaluate(async ([m, p, b]) =>
  (await fetch(p, { method: m, headers: { 'X-Recipes': '1', 'Content-Type': 'application/json' }, body: JSON.stringify(b) })).status, [method, path, body]);

// the desktop app is "Admin" and never signs in; it sets the family PIN
const admin = await (await browser.newContext({ viewport: { width: 1280, height: 820 } })).newPage();
watch(admin, 'admin');
await admin.goto(url);
await admin.waitForSelector('a.folder');
assert.equal(await admin.locator('.signin').count(), 0, 'the desktop app never signs in');
assert.equal(await call(admin, 'PUT', '/api/settings/pin', { pin: '123456' }), 204);

// a phone has to sign in: a new name and the PIN
const phone = await (await browser.newContext(devices['iPhone 13'])).newPage();
watch(phone, 'phone');
await phone.goto(origin + '/');
await phone.waitForSelector('.signin');
assert.equal(await call(phone, 'POST', '/api/messages', { text: 'x', expires: null }), 401, 'nobody can post before signing in');
await phone.screenshot({ path: `${out}/signin.png` });
await phone.fill('.signin input[type=password]', '000000');
await phone.fill('.signin input[aria-label="Your name"]', 'Ann');
await phone.fill('.signin input[aria-label="Full name"]', 'Ann Romo');
await phone.click('.signin button:has-text("Start")');
await phone.waitForSelector('.signin .error:has-text("wrong PIN")');
await phone.fill('.signin input[type=password]', '123456');
await phone.click('.signin button:has-text("Start")');
await phone.waitForSelector('.signin', { state: 'detached' });
await phone.reload();
await phone.waitForSelector('a.folder');
await phone.waitForTimeout(500);
assert.equal(await phone.locator('.signin').count(), 0, 'stays signed in');

// the whiteboard shows only with notes; one of each lifetime, seen live on the Mac
assert.equal(await phone.locator('.board').count(), 0, 'no whiteboard while empty');
const d = new Date(Date.now() + 3 * 86400e3);
const later = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
for (const [text, keep] of [['Out of eggs', 'today'], ['Party on the weekend', 'date'], ['Label the leftovers', 'forever']]) {
  await phone.click('button[aria-label="Write on the whiteboard"]');
  await phone.fill('.modal textarea', text);
  await phone.check(`.modal input[value="${keep}"]`);
  if (keep === 'date') await phone.fill('.modal input[type=date]', later);
  await phone.click('.modal button:has-text("Post")');
  await phone.waitForSelector(`.board .note:has-text("${text}")`);
}
await phone.screenshot({ path: `${out}/board-phone.png` });
await admin.waitForSelector('.board .note:has-text("Party on the weekend")');
assert.match(await admin.locator('.note:has-text("Out of eggs")').textContent(), /Today only/);
assert.match(await admin.locator('.note:has-text("Party on the weekend")').textContent(), /Until/);
await admin.screenshot({ path: `${out}/board-desktop.png` });
await phone.click('.note:has-text("Out of eggs") button[aria-label^="Remove"]');
await phone.click('.modal button[type=submit]');
await phone.waitForSelector('.note:has-text("Out of eggs")', { state: 'detached' });

// a favorite is the person's own, kept with their name on the Mac and shown on their page; a bookmark
// (everyone's) goes into the recipe
const sect = (title, card) => `.sect:has(h2:has-text("${title}")) .recipe:has-text("${card}")`;
const before = fs.readFileSync(pie, 'utf8');
await phone.goto(origin + '/#/n/Pies/Pumpkin%20Pie.md');
await phone.waitForSelector('.prose h1');
await phone.click('header button[aria-label="More"]');
await phone.click('.menu button:has-text("Add to favorites")');
await phone.goto(origin + '/#/');
await phone.waitForSelector(sect('Favorites', 'Pumpkin Pie'));
assert.equal(fs.readFileSync(pie, 'utf8'), before, 'a favorite leaves the recipe alone');
await admin.goto(origin + '/#/people/Ann%20Romo'); // no reload: her page hears about it live
await admin.waitForSelector(sect('Favorites', 'Pumpkin Pie'));
await admin.reload();
await admin.goto(origin + '/#/');
await admin.waitForSelector('a.folder');
assert.equal(await admin.locator('h2:has-text("Favorites")').count(), 0, 'favorites are the person’s own');
// renamed on the Mac, it stays her favorite
const annFavs = async () => (await phone.evaluate(async () => (await (await fetch('/api/users')).json()).find((u) => u.name === 'Ann').favorites));
// (her phone, on the home page, follows each rename live before the next one)
const favCard = (file) => `.sect:has(h2:has-text("Favorites")) a.recipe[href="#/n/Pies/${encodeURIComponent(file)}"]`;
assert.equal(await call(admin, 'POST', '/api/fs/move', { from: 'Pies/Pumpkin Pie.md', to: 'Pies/Pumpkin Pie Two.md' }), 200);
assert.deepEqual(await annFavs(), ['Pies/Pumpkin Pie Two.md'], 'a favorite follows a rename');
await phone.waitForSelector(favCard('Pumpkin Pie Two.md'));
assert.equal(await call(admin, 'POST', '/api/fs/move', { from: 'Pies/Pumpkin Pie Two.md', to: 'Pies/Pumpkin Pie.md' }), 200);
assert.deepEqual(await annFavs(), ['Pies/Pumpkin Pie.md']);
await phone.waitForSelector(favCard('Pumpkin Pie.md'));
await phone.goto(origin + '/#/n/Pies/Pumpkin%20Pie.md');
await phone.waitForSelector('.prose h1');
await phone.click('header button[aria-label="More"]');
await phone.click('.menu button:has-text("Bookmark for everyone")');
await admin.goto(origin + '/#/');
await admin.waitForSelector(sect('Bookmarks', 'Pumpkin Pie'));
assert.match(fs.readFileSync(pie, 'utf8'), /favorite: true/);

// ---------- recipe credits: who added, wrote and changed a recipe, in its properties ----------
const props = (file) => parseYaml(fs.readFileSync(file, 'utf8').match(/^---\n([\s\S]*?)\n---/)[1]);
const d0 = new Date();
const day = `${d0.getFullYear()}-${String(d0.getMonth() + 1).padStart(2, '0')}-${String(d0.getDate()).padStart(2, '0')}`;
const boston = `${vault}/Cakes/Boston Cream Pie.md`;

// Ann again on a laptop: an existing name, no full name asked
const laptop = await (await browser.newContext({ viewport: { width: 1280, height: 820 } })).newPage();
watch(laptop, 'laptop');
await laptop.goto(origin + '/');
await laptop.waitForSelector('.signin');
await laptop.fill('.signin input[type=password]', '123456');
await laptop.click('.signin .names button:has-text("Ann")');
await laptop.waitForSelector('.signin', { state: 'detached' });
// favorites this browser kept before they moved to the Mac join her list, once
const annId = await phone.evaluate(async () => (await (await fetch('/api/users')).json()).find((u) => u.name === 'Ann').id);
await laptop.evaluate((id) => localStorage.setItem(`favorites:${id}`, JSON.stringify(['Cakes/Boston Cream Pie.md'])), annId);
await laptop.reload();
await laptop.waitForSelector(sect('Favorites', 'Boston Cream Pie'));
assert.deepEqual(await annFavs(), ['Pies/Pumpkin Pie.md', 'Cakes/Boston Cream Pie.md'], 'carried over from this browser');
assert.equal(await laptop.evaluate((id) => localStorage.getItem(`favorites:${id}`), annId), null, 'and only once');

/** Type at the end of the open editor, save, answer the credit dialog. Returns whether the box started ticked. */
async function saveWith(page, typed, comment, credit) {
  await page.locator('.cm-content > .cm-line').last().click();
  await page.keyboard.press('Meta+End');
  await page.keyboard.type(typed);
  await page.keyboard.press('Meta+s');
  await page.waitForSelector('.modal .check');
  const ticked = await page.isChecked('.modal .check input');
  await page.setChecked('.modal .check input', credit);
  await page.fill('.modal input.field', comment);
  await page.click('.modal button[type=submit]');
  await page.waitForSelector('.modal', { state: 'detached' });
  await page.getByText('Saved', { exact: true }).waitFor();
  return ticked;
}
const until = async (check, what) => { for (let i = 0; i < 50 && !check(); i++) await new Promise((r) => setTimeout(r, 100)); assert.ok(check(), what); };

// someone else's recipe: the box starts unticked; the change is recorded, the author kept
await laptop.goto(origin + '/#/n/Cakes/Boston%20Cream%20Pie.md?mode=edit');
await laptop.waitForSelector('.cm-content');
assert.equal(await saveWith(laptop, '\nLess sugar next time', 'Less sugar', false), false, 'unticked for Kimberley’s recipe');
await until(() => fs.readFileSync(boston, 'utf8').includes('Less sugar next time'), 'edit saved');
assert.equal(props(boston).author, 'Kimberley Romo');
assert.deepEqual(props(boston).changes, [{ by: 'Ann Romo', date: day, note: 'Less sugar' }]);
// the cursor stayed at the end after the credits were written into the text
await laptop.keyboard.type(' and more');
assert.match(await laptop.locator('.cm-line').last().textContent(), /Less sugar next time and more$/);
assert.equal(await saveWith(laptop, '', 'Added a note', true), false);
await until(() => fs.readFileSync(boston, 'utf8').includes('and more'), 'second edit saved');
assert.deepEqual(props(boston).author, ['Kimberley Romo', 'Ann Romo']);
assert.deepEqual(props(boston).changes, [{ by: 'Ann Romo', date: day, note: 'Less sugar; Added a note' }], 'one entry per person per day');

// the reader: both authors under the title, the history under More; a detail edit joins the day's entry
await laptop.goto(origin + '/#/n/Cakes/Boston%20Cream%20Pie.md');
await laptop.waitForSelector('.byline:has-text("By Kimberley Romo and Ann Romo")');
await laptop.click('.props button.toggle');
await laptop.waitForSelector('.history li:has-text("Less sugar; Added a note")');
await laptop.click('button[aria-label="Add Prep"]');
await laptop.keyboard.type('20 min');
await laptop.keyboard.press('Enter');
await until(() => props(boston).prep_time === '20 min', 'prep time saved');
assert.equal(props(boston).changes[0].note, 'Less sugar; Added a note; Changed prep time');
await laptop.screenshot({ path: `${out}/credits.png` });

// her page lists what she wrote and changed; Kimberley, who never signs in, has a page too
await laptop.click('.byline a:has-text("Ann Romo")');
await laptop.waitForSelector('.hero h1:has-text("Ann Romo")');
await laptop.waitForSelector(sect('Recipes', 'Boston Cream Pie'));
await laptop.waitForSelector(sect('Favorites', 'Pumpkin Pie'));
await laptop.screenshot({ path: `${out}/person.png` });
await laptop.goto(origin + '/#/people');
await laptop.waitForSelector('.sect:has(h2:has-text("Family")) .person:has-text("Ann Romo")');
await laptop.click('.sect:has(h2:has-text("Also in the book")) .person:has-text("Kimberley Romo")');
await laptop.waitForSelector('.hero h1:has-text("Kimberley Romo")');
await laptop.waitForSelector(sect('Recipes', 'Boston Cream Pie'));
assert.equal(await laptop.locator('.hero button:has-text("picture")').count(), 0, 'no picture without an account');
// her own picture: an emoji; someone else can't change it
await laptop.goto(origin + '/#/people/Ann%20Romo');
await laptop.click('.hero button:has-text("Add a picture")');
await laptop.click('.modal button.emo >> nth=0');
await laptop.waitForSelector('.hero .av .emoji');
await laptop.waitForSelector('.modal', { state: 'detached' });
assert.equal(await call(phone, 'PUT', `/api/users/${annId}/picture`, { picture: '🥧' }), 204, 'her own, from her phone');
await laptop.goto(origin + '/#/people');
await laptop.waitForSelector('.person:has-text("Ann Romo") .av .emoji');
await laptop.screenshot({ path: `${out}/people-list.png` });

// a new recipe: its author and "Created"
await laptop.goto(origin + '/#/');
await laptop.click('button[aria-label="Add"]');
await laptop.click('.menu button:has-text("Blank recipe")');
await laptop.fill('.modal input.field', 'Test Soup');
await laptop.click('.modal button[type=submit]');
await until(() => fs.existsSync(`${vault}/Test Soup.md`), 'new recipe');
await laptop.waitForURL(/Test%20Soup\.md\?mode=edit/); // the app opens it
assert.equal(props(`${vault}/Test Soup.md`).author, 'Ann Romo');
assert.equal(props(`${vault}/Test Soup.md`).changes[0].note, 'Created');

// an import: who added it and where from; the author stays the recipe's own
await laptop.goto(origin + '/#/import');
await laptop.click('button:has-text("Paste recipe text")');
await laptop.fill('textarea[placeholder="Paste the recipe here…"]', 'Pasted Stew\n\nIngredients\n2 cups broth\n1 onion\n\nDirections\n1. Simmer for an hour.\n');
await laptop.click('button:has-text("Use this text")');
await laptop.waitForSelector('.rev input.title', { timeout: 30000 });
await laptop.fill('.rev input.title', 'Pasted Stew');
await laptop.click('button:has-text("Save recipe")');
await laptop.waitForSelector('.prose h1', { timeout: 20000 });
const stew = fs.readdirSync(vault, { recursive: true }).find((f) => String(f).endsWith('Pasted Stew.md'));
assert.ok(stew, 'imported recipe saved');
assert.equal(props(`${vault}/${stew}`).added_by, 'Ann Romo');
assert.equal(props(`${vault}/${stew}`).changes[0].note, 'Imported from pasted text');
assert.notEqual(props(`${vault}/${stew}`).author, 'Ann Romo');

// the Mac edits as Admin until someone is chosen beside Save, then credits that person
await admin.goto(origin + '/#/n/Pies/Pumpkin%20Pie.md?mode=edit');
await admin.waitForSelector('.cm-content');
assert.equal(await admin.locator('.toolbar .editing-as select option:checked').textContent(), 'Admin', 'Admin by default');
await admin.selectOption('.toolbar .editing-as select', { label: 'Ann' });
await admin.click('.cm-content');
await admin.keyboard.press('Meta+End');
await admin.keyboard.type('\nFrom the Mac');
await admin.keyboard.press('Meta+s');
await admin.waitForSelector('.modal .check');
await admin.fill('.modal input.field', 'Typed on the Mac');
await admin.click('.modal button[type=submit]');
await admin.getByText('Saved', { exact: true }).waitFor();
await until(() => fs.readFileSync(pie, 'utf8').includes('From the Mac'), 'Mac edit saved');
assert.equal(props(pie).changes.at(-1).note, 'Typed on the Mac');
assert.equal(props(pie).changes.at(-1).by, 'Ann Romo');
assert.equal(await admin.locator('.toolbar .editing-as select option:checked').textContent(), 'Ann', 'Editing as remembered');

// the Mac's People page: Ann, her phone, what she changed
await admin.goto(origin + '/#/settings/people');
await admin.click('summary:has-text("Ann")');
await admin.waitForSelector('.log li:has-text("Safari on iPhone")');
await admin.waitForSelector('.log li:has-text("Saved Pies/Pumpkin Pie.md")');
await admin.waitForSelector('.log li:has-text("Wrote on the whiteboard")');
await admin.waitForSelector('.log li:has-text("Removed a whiteboard note")');
await admin.screenshot({ path: `${out}/people.png` });

// removing her signs the phone out
await admin.click('details[open] button:has-text("Remove")');
await admin.click('.modal button[type=submit]');
await admin.waitForSelector('summary:has-text("Ann")', { state: 'detached' });
await phone.reload();
await phone.waitForSelector('.signin');

await browser.close();
assert.deepEqual(errors, []);
console.log('accounts ok');
