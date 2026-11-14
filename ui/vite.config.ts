import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { VitePWA } from 'vite-plugin-pwa';

// `npm run dev` proxies the API to a headless recipes-server on :8640.
export default defineConfig({
  plugins: [
    svelte(),
    VitePWA({
      registerType: 'autoUpdate',
      injectRegister: false, // registered in main.ts (skipped inside the desktop app)
      includeAssets: ['icon.svg', 'apple-touch-icon.png', 'theme.js'],
      manifest: {
        name: 'Family Recipes',
        short_name: 'Recipes',
        description: 'The family cookbook',
        start_url: '/',
        scope: '/',
        display: 'standalone',
        background_color: '#fafaf8',
        theme_color: '#fafaf8',
        icons: [
          { src: '/icon-192.png', sizes: '192x192', type: 'image/png' },
          { src: '/icon-512.png', sizes: '512x512', type: 'image/png' },
          { src: '/icon-maskable-512.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
          { src: '/icon.svg', sizes: 'any', type: 'image/svg+xml' },
        ],
      },
      workbox: {
        globPatterns: ['**/*.{js,css,html,svg,png,mjs}'],
        maximumFileSizeToCacheInBytes: 4 * 1024 * 1024,
        navigateFallback: '/index.html',
        navigateFallbackDenylist: [/^\/api\//, /^\/auth/, /^\/ca\.crt/],
        runtimeCaching: [
          // the whole vault (titles, text, recipes) -> offline reading + search
          { urlPattern: ({ url }) => url.pathname === '/api/vault' || url.pathname === '/api/session' || url.pathname === '/api/ingredients', handler: 'NetworkFirst',
            options: { cacheName: 'vault', networkTimeoutSeconds: 4 } },
          { urlPattern: ({ url }) => url.pathname.startsWith('/api/files/'), handler: 'StaleWhileRevalidate',
            options: { cacheName: 'files', expiration: { maxEntries: 400 } } },
        ],
      },
    }),
  ],
  build: { outDir: 'dist', emptyOutDir: true, chunkSizeWarningLimit: 2000 },
  server: { proxy: { '/api': 'http://127.0.0.1:8640', '/auth': 'http://127.0.0.1:8640' } },
});
