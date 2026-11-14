import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';

mount(App, { target: document.getElementById('app')! });

// Offline-capable PWA for phones/tablets. Not inside the desktop app (loopback), where a
// cached UI would only get in the way of updates.
if ('serviceWorker' in navigator && location.hostname !== '127.0.0.1' && location.hostname !== 'localhost') {
  import('virtual:pwa-register').then(({ registerSW }) => registerSW({ immediate: true }));
}
