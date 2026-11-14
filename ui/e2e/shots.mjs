// Quick visual check: node e2e/shots.mjs <admin-url> <outdir>
import { chromium, devices } from '@playwright/test';
const [url, out] = process.argv.slice(2);
const browser = await chromium.launch();
const errors = [];
for (const [name, opts] of [['desktop', { viewport: { width: 1280, height: 820 } }], ['phone', devices['iPhone 13']]]) {
  const ctx = await browser.newContext(opts);
  const page = await ctx.newPage();
  page.on('pageerror', (e) => errors.push(`${name}: ${e.message}`));
  page.on('console', (m) => m.type() === 'error' && errors.push(`${name} console: ${m.text()}`));
  await page.goto(url);
  await page.waitForSelector('a.folder');
  await page.screenshot({ path: `${out}/${name}-home.png` });
  await page.goto(new URL('/#/n/Cakes/Boston%20Cream%20Pie.md', url).href);
  await page.waitForSelector('.prose h1');
  await page.screenshot({ path: `${out}/${name}-note.png` });
  await page.click('button[aria-label="More"] >> nth=0').catch(() => {});
  await page.keyboard.press('Escape');
}
await browser.close();
console.log(errors.length ? errors.join('\n') : 'no errors');
