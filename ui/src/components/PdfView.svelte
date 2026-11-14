<script lang="ts">
  import { onMount } from 'svelte';
  import { fileUrl } from '../lib/api';

  // pdf.js renders identically in the Tauri webview, iOS standalone PWAs and Android Chrome,
  // where a native <iframe>/<embed> PDF often shows only the first page or nothing.
  let { path }: { path: string } = $props();
  let host: HTMLDivElement;
  let status = $state('Loading PDF…');
  let zoom = $state(1);
  let doc: any = null;

  async function render() {
    if (!doc) return;
    host.replaceChildren();
    const width = Math.min(host.clientWidth || 800, 1000);
    for (let i = 1; i <= doc.numPages; i++) {
      const page = await doc.getPage(i);
      const base = page.getViewport({ scale: 1 });
      const scale = (width / base.width) * zoom;
      const vp = page.getViewport({ scale: scale * devicePixelRatio });
      const canvas = document.createElement('canvas');
      canvas.width = vp.width;
      canvas.height = vp.height;
      canvas.style.width = vp.width / devicePixelRatio + 'px';
      canvas.setAttribute('aria-label', `Page ${i}`);
      host.appendChild(canvas);
      await page.render({ canvas, canvasContext: canvas.getContext('2d')!, viewport: vp }).promise;
    }
  }

  onMount(() => {
    let cancelled = false;
    (async () => {
      try {
        const pdfjs = await import('pdfjs-dist');
        const worker = (await import('pdfjs-dist/build/pdf.worker.min.mjs?url')).default;
        pdfjs.GlobalWorkerOptions.workerSrc = worker;
        doc = await pdfjs.getDocument({ url: fileUrl(path) }).promise;
        if (cancelled) return;
        status = '';
        await render();
      } catch (e: any) {
        status = 'Could not display this PDF: ' + (e?.message ?? e);
      }
    })();
    return () => { cancelled = true; doc?.destroy?.(); };
  });
</script>

<div class="bar row no-print">
  <button class="btn" onclick={() => { zoom = Math.max(0.5, zoom - 0.25); render(); }} aria-label="Zoom out">－</button>
  <span class="small muted">{Math.round(zoom * 100)}%</span>
  <button class="btn" onclick={() => { zoom = Math.min(3, zoom + 0.25); render(); }} aria-label="Zoom in">＋</button>
  <a class="btn" href={fileUrl(path)} target="_blank" rel="noopener">Open in new tab</a>
</div>
{#if status}<p class="muted">{status}</p>{/if}
<div class="pages" bind:this={host}></div>

<style>
  .bar { margin-bottom: 10px; }
  .pages { display: grid; gap: 12px; justify-items: center; overflow-x: auto; }
  .pages :global(canvas) { box-shadow: var(--shadow-md); background: #fff; max-width: none; }
</style>
