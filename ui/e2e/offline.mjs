// PWA offline check over the LAN HTTPS listener: node e2e/offline.mjs <stop-command>
import { chromium, devices } from '@playwright/test';
import { execSync } from 'node:child_process';
const browser = await chromium.launch({ args: ['--ignore-certificate-errors'] });
const ctx = await browser.newContext({ ...devices['iPhone 13'], ignoreHTTPSErrors: true });
const page = await ctx.newPage();
await page.goto('https://recipes.local/');
await page.waitForSelector('a.folder');
await page.evaluate(() => navigator.serviceWorker.ready);
await page.reload();
await page.waitForSelector('a.folder');
const controlled = await page.evaluate(() => !!navigator.serviceWorker.controller);
await page.goto('https://recipes.local/#/n/Pies/Pumpkin%20Pie.md');
await page.waitForSelector('.prose h1');
execSync(process.argv[2]);           // server goes away (Mac asleep)
await ctx.setOffline(true);
await page.goto('https://recipes.local/');
await page.waitForSelector('a.folder', { timeout: 10000 });
await page.goto('https://recipes.local/#/n/Cakes/Boston%20Cream%20Pie.md');
await page.waitForSelector('.prose h1');
await page.screenshot({ path: '/tmp/offline.png' });
console.log('controlled', controlled, '— offline reading ok');
await browser.close();
