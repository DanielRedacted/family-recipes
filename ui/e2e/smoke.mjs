// Smoke check before a release: the app loads, shows its folders and opens a recipe, at desktop and
// phone width, with no script errors. Read-only. node e2e/smoke.mjs <admin-url>
import { chromium } from '@playwright/test';
const [url] = process.argv.slice(2);
const origin = new URL(url).origin;
const browser = await chromium.launch();
const errors = [];
try {
  const context = await browser.newContext({ viewport: { width: 1280, height: 820 } });
  const page = await context.newPage();
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(url); // sets the admin cookie
  const r = await page.request.put(`${origin}/api/settings`, { data: { setup_done: true }, headers: { 'X-Recipes': '1', Origin: origin } });
  if (!r.ok()) throw new Error(`PUT /api/settings: ${r.status()}`);
  await page.goto(origin);
  await page.waitForSelector('a.folder');
  await page.keyboard.press('Meta+k');
  await page.fill('input[aria-label="Search"]', 'pumpkin pie');
  await page.waitForSelector('li[role=option] button.sel');
  await page.keyboard.press('Enter');
  await page.waitForSelector('.prose h1:has-text("Pumpkin Pie")');
  // a fresh context, like a second device: the first one reopens its last tab
  const phone = await (await browser.newContext({ viewport: { width: 390, height: 844 } })).newPage();
  phone.on('pageerror', (e) => errors.push(e.message));
  await phone.goto(url);
  await phone.waitForSelector('a.folder');
  await phone.waitForSelector('nav.bottombar'); // the phone layout, not the desktop one
  if (errors.length) throw new Error(`script errors:\n${errors.join('\n')}`);
  console.log('smoke: ok');
} finally {
  await browser.close();
}
