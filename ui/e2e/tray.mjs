// Menu-bar template icon (black on transparent, 44px for retina).
import { chromium } from '@playwright/test';
const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 44, height: 44 } });
await p.setContent(`<body style="margin:0"><svg width="44" height="44" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/><circle cx="13" cy="10" r="3"/></svg></body>`);
await p.screenshot({ path: '../src-tauri/icons/tray.png', omitBackground: true }); await b.close();
