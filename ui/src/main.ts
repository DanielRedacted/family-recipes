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
  const https = 'https://' + location.hostname; // ponytail: probes 443 only, not the 8443 fallback used when 443 is taken
  fetch(https + '/api/ping', { mode: 'no-cors', cache: 'no-store' })
    .then(() => location.replace(https + location.pathname + location.search + location.hash))
    .catch(() => {});
}

// Offline-capable PWA for phones/tablets. Not inside the desktop app (loopback), where a
// cached UI would only get in the way of updates.
if ('serviceWorker' in navigator && !loopback) {
  import('virtual:pwa-register').then(({ registerSW }) => registerSW({ immediate: true }));
}
