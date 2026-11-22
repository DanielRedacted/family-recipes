// The Calendar page, today's events on the home page and guessed meals:
//   node e2e/calendar.mjs <admin-url> <vault-dir> <outdir>
// A headless server can't read Calendar.app, so /api/calendar and /api/calendar/calendars are answered
// here (that also keeps macOS's permission prompt from appearing). Everything else is the real server.
import { chromium } from '@playwright/test';
import fs from 'node:fs';
import assert from 'node:assert/strict';
const [url, , out] = process.argv.slice(2);
fs.mkdirSync(out, { recursive: true });
const browser = await chromium.launch();
const errors = [];

const midnight = (d = new Date(), n = 0) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const at = (h, m = 0, n = 0) => Math.floor(new Date(midnight(new Date(), n).getTime() + (h * 60 + m) * 60_000).getTime() / 1000);
const EVENTS = [
  { title: 'Grandma’s birthday', start: at(0), end: at(23, 59), all_day: true, calendar: 'Family', location: null },
  { title: 'Soccer practice', start: at(16), end: at(17, 30), all_day: false, calendar: 'Family', location: 'Lincoln Park' },
  { title: 'Dinner with the Parks', start: at(18, 30), end: at(21), all_day: false, calendar: 'Home', location: null },
  { title: 'Ski trip', start: at(9, 0, 2), end: at(0, 0, 5), all_day: false, calendar: 'Family', location: 'Tahoe' },
];
let events = EVENTS;

async function open(opts) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 860 }, ...opts });
  // the app is light unless Settings → General says Dark (it doesn't follow the system)
  if (opts?.colorScheme === 'dark') await context.addInitScript(() => localStorage.setItem('theme', 'dark'));
  const page = await context.newPage();
  page.on('pageerror', (e) => errors.push(e.message));
  await page.route(/\/api\/calendar\/calendars/, (r) => r.fulfill({ json: { access: 'ok', calendars: [{ id: 'f', title: 'Family' }, { id: 'h', title: 'Home' }, { id: 'w', title: 'Work' }] } }));
  await page.route(/\/api\/calendar\?/, (r) => {
    const q = new URL(r.request().url()).searchParams;
    r.fulfill({ json: events.filter((e) => e.start < +q.get('to') && e.end > +q.get('from')) });
  });
  return { context, page };
}
const call = (page, method, path, body) => page.evaluate(async ([m, p, b]) => {
  const r = await fetch(p, { method: m, headers: { 'X-Recipes': '1', 'Content-Type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });
  return r.json();
}, [method, path, body]);

// ---------- desktop, light ----------
let { context, page } = await open();
await page.clock.install({ time: new Date() });
await page.goto(url);
await call(page, 'PUT', '/api/settings', { setup_done: true });
await page.reload();
await page.waitForSelector('a.folder');
assert.equal(await page.locator('.today').count(), 0, 'no Today list while the calendar is off');

// switching it on shows today's events on the home page
await call(page, 'PUT', '/api/settings', { calendar: { enabled: true, calendars: [] } });
assert.equal((await call(page, 'GET', '/api/session')).calendar, false, 'on, but no calendar ticked yet: nothing shows');
await call(page, 'PUT', '/api/settings', { calendar: { enabled: true, calendars: ['f', 'h'] } });
await page.reload();
await page.waitForSelector('.today li');
assert.equal(await page.locator('.today li').count(), 3);
await page.screenshot({ path: `${out}/home-today.png` });

// no events today: the list goes away
events = EVENTS.slice(3);
await page.reload();
await page.waitForSelector('a.folder');
await page.clock.runFor(61_000);
assert.equal(await page.locator('.today').count(), 0, 'Today hides when today has no events');
events = EVENTS;

// a recipe left open for 20 minutes is logged as a guess, once
await page.goto(url.replace(/\/auth.*/, '/#/n/Pies/Pumpkin%20Pie.md'));
await page.waitForSelector('.prose h1');
await page.clock.runFor(19 * 60_000);
assert.equal((await call(page, 'GET', '/api/made')).length, 0, 'no guess before 20 minutes');
await page.clock.runFor(2 * 60_000);
await page.waitForFunction(async () => (await (await fetch('/api/made')).json()).length === 1);
let made = await call(page, 'GET', '/api/made');
assert.equal(made[0].assumed, true);
await page.clock.runFor(30 * 60_000);
assert.equal((await call(page, 'GET', '/api/made')).length, 1, 'only one guess per opening');
// the button still says "Made today", not "Made today ✓"
assert.ok(await page.locator('.made button:not([disabled])').count(), 'a guess leaves Made today pressable');

// the Calendar page shows it as a guess; "Yes, cooked" makes it a fact
await page.click('button:has-text("Calendar")');
await page.waitForSelector('.week');
await page.waitForSelector('.ev');
assert.ok(await page.locator('.log.guess:has-text("Probably cooked")').count());
await page.screenshot({ path: `${out}/week-guess.png`, fullPage: true });
await page.click('button:has-text("Yes, cooked")');
await page.waitForSelector('.log:not(.guess):has-text("Cooked")');
made = await call(page, 'GET', '/api/made');
assert.equal(made[0].assumed, undefined);

// a second guess, removed with "Not cooked"
await page.goto(url.replace(/\/auth.*/, '/#/n/Pies/Pumpkin%20Pie.md'));
await call(page, 'POST', '/api/made', { path: 'Pies/Pumpkin Pie.md', assumed: true });
assert.equal((await call(page, 'GET', '/api/made')).length, 1, 'no guess on a day it was cooked');
await call(page, 'POST', '/api/made', { path: 'Breads/' + fs.readdirSync(`${process.argv[3]}/Breads`).find((f) => f.endsWith('.md')), assumed: true }).catch(() => {});
await page.goto(url.replace(/\/auth.*/, '/#/calendar'));
await page.waitForSelector('.week');
const guesses = await page.locator('.log.guess').count();
if (guesses) {
  await page.click('.log.guess button:has-text("Not cooked")');
  await page.waitForFunction((n) => document.querySelectorAll('.log.guess').length === n - 1, guesses);
}

// month view, then the chosen day's list
await page.click('.seg button:has-text("Month")');
await page.waitForSelector('.month .cell');
await page.waitForSelector('.pk.ev');
await page.screenshot({ path: `${out}/month.png`, fullPage: true });
assert.ok(await page.locator('.cell.picked.today').count(), 'today is chosen');

// Settings → Calendar lists the calendars
await page.goto(url.replace(/\/auth.*/, '/#/settings/calendar'));
await page.waitForSelector('.cals label');
assert.equal(await page.locator('.cals input:checked').count(), 2, 'only the ticked calendars are ticked');
await page.locator('.cals label:has-text("Work") input').check();
await page.waitForFunction(async () => (await (await fetch('/api/settings')).json()).calendar.calendars.includes('w'));
await page.locator('.cals label:has-text("Home") input').uncheck();
await page.waitForFunction(async () => !(await (await fetch('/api/settings')).json()).calendar.calendars.includes('h'));
await page.screenshot({ path: `${out}/settings.png` });
await context.close();

// ---------- phone, light and dark ----------
for (const scheme of ['light', 'dark']) {
  ({ context, page } = await open({ viewport: { width: 390, height: 844 }, colorScheme: scheme, hasTouch: true, isMobile: true }));
  await page.goto(url);
  await page.waitForSelector('a.folder');
  await page.waitForSelector('.today li');
  await page.screenshot({ path: `${out}/phone-home-${scheme}.png` });
  await page.goto(url.replace(/\/auth.*/, '/#/calendar'));
  await page.click('.seg button:has-text("Week")');
  await page.waitForSelector('.ev');
  await page.screenshot({ path: `${out}/phone-week-${scheme}.png`, fullPage: true });
  await page.click('.seg button:has-text("Month")');
  await page.waitForSelector('.month .cell');
  await page.screenshot({ path: `${out}/phone-month-${scheme}.png`, fullPage: true });
  await context.close();
}

// desktop dark
({ context, page } = await open({ colorScheme: 'dark' }));
await page.goto(url);
await page.waitForSelector('a.folder');
await page.evaluate(() => (location.hash = '#/calendar'));
await page.click('.seg button:has-text("Week")');
await page.waitForSelector('.ev');
await page.screenshot({ path: `${out}/week-dark.png`, fullPage: true });
await context.close();

await browser.close();
assert.deepEqual(errors, [], 'page errors');
console.log('calendar flows ok');
