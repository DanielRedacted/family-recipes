<script lang="ts" module>
  import { app, wide } from '../lib/state.svelte';

  // Feedback goes to FEEDBACK.md at the top of the recipes folder (crates/core/src/feedback.rs).
  // No button for it anywhere: search "feedback" (or bug, idea…), press ⌘⇧F, or use Settings → Help.
  export const fb = $state({ open: false, context: '' });

  /** Search words that offer "Send feedback" in the switcher. */
  export const feedbackWords = /^(feedback|bug|idea|suggest|report|problem|broken)/i;

  /** Open the form, noting the page, any selected text and the device, read now before focus moves. */
  export function openFeedback() {
    const r = app.route;
    const page = r.view === 'note' ? app.byPath[r.path]?.title ?? r.path
      : r.view === 'folder' ? (r.path ? `Folder ${r.path}` : 'Home')
      : `${r.view[0].toUpperCase()}${r.view.slice(1)}${r.path ? ' › ' + r.path : ''}`;
    const sel = (getSelection()?.toString() ?? '').trim().replace(/\s+/g, ' ').slice(0, 300);
    const ua = navigator.userAgent;
    const browser = /Edg\//.test(ua) ? 'Edge' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : 'browser';
    const device = app.session.shell ? 'Mac app' : `${wide() ? 'computer' : 'phone'} · ${browser}`;
    fb.context = [
      `- Page: ${page} (${location.hash || '#/'})`,
      sel && `- Selected: “${sel}”`,
      `- Device: ${device} · ${innerWidth}×${innerHeight} · app ${app.session.version}`,
    ].filter(Boolean).join('\n');
    fb.open = true;
  }
</script>

<script lang="ts">
  import { api } from '../lib/api';
  import { toast } from '../lib/state.svelte';
  import { modal } from '../lib/modal';

  const kinds = [
    { value: 'Bug', label: 'Something’s wrong', hint: 'What happened, and what did you expect?' },
    { value: 'Idea', label: 'An idea', hint: 'What would make the app better?' },
    { value: 'Other', label: 'Other', hint: 'Anything else on your mind' },
  ];
  let kind = $state('Bug');
  let text = $state('');
  let include = $state(true);
  let busy = $state(false);
  let error = $state(''); // shown in the form: a toast would sit under the dialog

  function close() { fb.open = false; text = ''; kind = 'Bug'; include = true; error = ''; }

  async function send(e: Event) {
    e.preventDefault();
    if (busy || !text.trim()) return;
    busy = true;
    error = '';
    try {
      await api('POST', '/api/feedback', { kind, text, context: include ? fb.context : '' });
      close();
      toast('Thanks, feedback sent', 'ok');
    } catch (err: any) { error = err.message; } finally { busy = false; }
  }

  // ⌘↩ / Ctrl+↩ sends (needs no IME check, unlike a bare Enter)
  const key = (e: KeyboardEvent) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) (e.currentTarget as HTMLTextAreaElement).form?.requestSubmit(); };
  const focus = (el: HTMLElement) => { setTimeout(() => el.focus(), 0); };
</script>

{#if fb.open}
  <dialog class="modal-host" aria-labelledby="fb-title" use:modal={{ onclose: close, outside: false }}>
    <form class="modal card" onsubmit={send}>
      <h3 id="fb-title">Send feedback</h3>
      <fieldset class="kinds" aria-label="Kind of feedback">
        {#each kinds as k}
          <label class="kind"><input type="radio" bind:group={kind} value={k.value} /> {k.label}</label>
        {/each}
      </fieldset>
      <textarea id="fb-text" aria-label="Your feedback" class="field" rows="5" maxlength="5000" bind:value={text} onkeydown={key} use:focus placeholder={kinds.find((k) => k.value === kind)?.hint}></textarea>
      <label class="row check"><input type="checkbox" bind:checked={include} /> Include what I was looking at</label>
      {#if include}<p class="ctx muted small">{fb.context.replace(/^- /gm, '')}</p>{/if}
      {#if error}<p class="form-error" role="alert">{error}</p>{/if}
      <div class="row end">
        <button type="button" class="btn" onclick={close}>Cancel</button>
        <button class="btn primary" disabled={busy || !text.trim()}>Send feedback</button>
      </div>
    </form>
  </dialog>
{/if}

<style>
  .modal { width: min(480px, calc(100vw - 32px)); padding: 20px; display: grid; gap: 12px; }
  .modal h3 { margin: 0; }
  .kinds { border: 0; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 4px 16px; }
  .kind { display: inline-flex; align-items: center; gap: 6px; min-height: 36px; }
  /* grows with the text where field-sizing is supported; a fixed 5 rows with a scrollbar elsewhere */
  textarea { resize: vertical; font: inherit; width: 100%; field-sizing: content; min-height: 5lh; max-height: 14lh; }
  .check { gap: 8px; }
  .ctx { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font: inherit; max-height: 7lh; overflow-y: auto; }
  .end { justify-content: flex-end; }
</style>
