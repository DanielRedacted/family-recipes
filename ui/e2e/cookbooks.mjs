// Two cookbooks, each device in its own: node e2e/cookbooks.mjs <admin-url> <vault-dir> <outdir> <second-vault-dir>
// The second folder needs a name of its own (not the first's) and must sit outside the first. The script
// writes Pies/Pumpkin Pie.md into it, a path the first cookbook has too, so every check below would
// catch a request answered from the wrong cookbook. Run on copies.
import { chromium, devices } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const [url, vault, out, vault2] = process.argv.slice(2);
const origin = new URL(url).origin;
const PIE = '#/n/Pies/Pumpkin%20Pie.md';
const mine = path.basename(vault2);
fs.mkdirSync(`${vault2}/Pies`, { recursive: true });
fs.writeFileSync(`${vault2}/Pies/Pumpkin Pie.md`, '---\ntags: [Dessert]\n---\n# My Pumpkin Pie\n\n## Ingredients\n\n- 1 cup pumpkin\n\n## Directions\n\n1. Bake.\n');
const browser = await chromium.launch();
const errors = [];
const watch = (p, who) => p.on('pageerror', (e) => errors.push(`${who}: ${e.message}`));
const call = (page, method, p, body) => page.evaluate(async ([m, p, b]) => {
  const r = await fetch(p, { method: m, headers: { 'X-Recipes': '1', 'Content-Type': 'application/json' }, body: b === undefined ? undefined : JSON.stringify(b) });
  return { status: r.status, body: await r.text() };
}, [method, p, body]);
const title = (page) => page.locator('.prose h1').first().innerText();

// the Mac: one cookbook, so no switcher, as before cookbooks
const admin = await (await browser.newContext({ viewport: { width: 1280, height: 820 } })).newPage();
watch(admin, 'admin');
await admin.goto(url);
assert.equal((await call(admin, 'PUT', '/api/settings', { setup_done: true })).status, 200);
assert.equal((await call(admin, 'PUT', '/api/settings/pin', { pin: '123456' })).status, 204);
await admin.goto(origin + '/');
await admin.waitForSelector('a.folder');
assert.equal(await admin.locator('select.cookbook').count(), 0, 'one cookbook: no switcher');

// a second cookbook, opened live (no restart); a folder inside the first is refused
assert.equal((await call(admin, 'POST', '/api/cookbooks', { path: `${vault}/Pies` })).status, 400);
const added = await call(admin, 'POST', '/api/cookbooks', { path: vault2 });
assert.equal(added.status, 200, added.body);
const id = JSON.parse(added.body).id;
await admin.reload();
await admin.waitForSelector('select.cookbook');
// ("Manage cookbooks…" is added only in the desktop app)
assert.deepEqual(await admin.locator('select.cookbook option:not([disabled])').allInnerTexts(), [path.basename(vault), mine]);
await admin.screenshot({ path: `${out}/switcher.png` });

// a phone signs in and switches to the second cookbook from the drawer
const phone = await (await browser.newContext(devices['iPhone 13'])).newPage();
watch(phone, 'phone');
await phone.goto(origin + '/');
await phone.waitForSelector('.signin');
await phone.fill('.signin input[type=password]', '123456');
await phone.fill('.signin input[aria-label="Your name"]', 'Ann');
await phone.fill('.signin input[aria-label="Full name"]', 'Ann Romo');
await phone.click('.signin button:has-text("Start")');
await phone.waitForSelector('.signin', { state: 'detached' });
await phone.waitForSelector('a.folder');
await phone.click('nav.bottombar button:has-text("Folders")');
await Promise.all([phone.waitForEvent('load'), phone.selectOption('select.cookbook', id)]);
await phone.waitForSelector('a.folder');
assert.equal(await phone.locator('.bar-name').first().innerText(), mine, 'the home page is named after its cookbook');
await phone.screenshot({ path: `${out}/phone-home.png` });

// the same address is a different recipe in each cookbook
await admin.goto(origin + '/' + PIE);
await phone.goto(origin + '/' + PIE);
await admin.waitForSelector('.prose h1');
await phone.waitForSelector('.prose h1:has-text("My Pumpkin Pie")');
assert.equal(await title(admin), 'Pumpkin Pie');

// a favorite and a meal from the phone are kept under its cookbook
await phone.click('header button[aria-label="More"]');
await phone.click('.menu button:has-text("Add to favorites")');
await phone.click('.made button');
await phone.click('dialog[open] button:has-text("Made today")');
await phone.waitForSelector('.made button[disabled]');
const users = JSON.parse(fs.readFileSync(`${process.env.DATA}/users.json`, 'utf8'));
const ann = users.find((u) => u.name === 'Ann');
assert.deepEqual([ann.favorites, ann.favorites_in?.[id]], [[], ['Pies/Pumpkin Pie.md']]);
const made = JSON.parse(fs.readFileSync(`${process.env.DATA}/made.json`, 'utf8'));
assert.deepEqual(made.map((m) => [m.book, m.title]), [[id, 'My Pumpkin Pie']]);
// …so the Mac's Pumpkin Pie isn't "made today", and isn't Ann's favorite
await admin.reload();
await admin.waitForSelector('.made button:not([disabled])');

// the Mac's Calendar shows the meal, named with its cookbook, not linked to its own Pumpkin Pie
await admin.goto(origin + '/#/calendar');
const meal = admin.locator('li.log', { hasText: 'My Pumpkin Pie' }).first();
await meal.waitFor();
assert.equal(await meal.locator('a').count(), 0, "another cookbook's meal is not a link here");
assert.match(await meal.innerText(), new RegExp(mine));

// a save in one cookbook lands only in its folder, and only its readers hear about it
const before = fs.readFileSync(`${vault}/Pies/Pumpkin Pie.md`, 'utf8');
const put = await phone.evaluate(async (b) => (await fetch(`/api/files/Soup.md?book=${b}`, { method: 'PUT', headers: { 'X-Recipes': '1', 'If-None-Match': '*' }, body: '# Soup\n' })).status, id);
assert.equal(put, 200);
assert.ok(fs.existsSync(`${vault2}/Soup.md`) && !fs.existsSync(`${vault}/Soup.md`));
await phone.goto(origin + '/#/');
await phone.waitForSelector('a:has-text("Soup"), .recipe:has-text("Soup")');
await admin.goto(origin + '/#/');
await admin.waitForSelector('a.folder');
assert.equal(await admin.locator('.recipe:has-text("Soup")').count(), 0);

// an unknown cookbook is refused, never answered from another: nothing is trashed
assert.equal((await call(phone, 'DELETE', '/api/files/Pies/Pumpkin%20Pie.md?book=nope')).status, 404);
assert.equal(fs.readFileSync(`${vault}/Pies/Pumpkin Pie.md`, 'utf8'), before);
assert.ok(fs.existsSync(`${vault2}/Pies/Pumpkin Pie.md`));

// the phone's choice survives a reload; removing the cookbook moves it back to the first
await phone.goto(origin + '/' + PIE);
await phone.waitForSelector('.prose h1:has-text("My Pumpkin Pie")');
assert.equal((await call(admin, 'DELETE', `/api/cookbooks/${id}`)).status, 204);
assert.ok(fs.existsSync(`${vault2}/Pies/Pumpkin Pie.md`), 'its folder stays');
await phone.reload();
await phone.waitForSelector('a.folder');
await phone.goto(origin + '/' + PIE);
await phone.waitForSelector('.prose h1');
assert.equal(await title(phone), 'Pumpkin Pie');
await admin.reload();
await admin.waitForSelector('a.folder');
assert.equal(await admin.locator('select.cookbook').count(), 0, 'one cookbook again: no switcher');

assert.deepEqual(errors, []);
await browser.close();
console.log('cookbooks: ok');
