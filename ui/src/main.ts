import { mount } from 'svelte';
import './prose.css';
import './app.css';
import App from './App.svelte';

mount(App, { target: document.getElementById('app')! });

// no pinch zoom: iOS Safari ignores user-scalable=no in the viewport tag
document.addEventListener('gesturestart', (e) => e.preventDefault());

// Plain HTTP on the home network needs no setup. A device that already trusts the Recipes
// certificate moves to HTTPS (where the offline app lives); untrusted, the probe fails and we stay.
const loopback = location.hostname === '127.0.0.1' || location.hostname === 'localhost';
if (location.protocol === 'http:' && !loopback) {
  // 443, else the 8443 the Mac falls back to when another app holds 443 (that app fails the certificate check)
  const go = (https: string) => location.replace(https + location.pathname + location.search + location.hash);
  const probe = (https: string) => fetch(https + '/api/ping', { mode: 'no-cors', cache: 'no-store' }).then(() => https);
  const host = 'https://' + location.hostname;
  probe(host).catch(() => probe(host + ':8443')).then(go, () => {});
}

// Offline-capable PWA for phones/tablets. Not inside the desktop app (loopback), where a
// cached UI would only get in the way of updates.
if ('serviceWorker' in navigator && !loopback) {
  import('virtual:pwa-register').then(({ registerSW }) => registerSW({ immediate: true }));
}
