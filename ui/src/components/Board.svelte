<script lang="ts">
  // The kitchen whiteboard on the home page: shown only while it has notes. The compose form opens
  // from the home page's toolbar (board.compose), so it works while the board is hidden.
  import Icon from './Icon.svelte';
  import { api } from '../lib/api';
  import { app, toast } from '../lib/state.svelte';
  import { board, loadMessages, endOfDay, today } from '../lib/board.svelte';
  import { confirmIt } from '../lib/dialog.svelte';
  import { getContext } from 'svelte';
  import type { Tab } from '../lib/state.svelte';
  import { modal } from '../lib/modal';
  import Section from './Section.svelte';
  import { nameOnRecipes, personHash } from '../lib/people';
  import { renderInline } from '../lib/markdown';

  const tab = getContext<Tab>('tab'); // the home page can be open in more than one tab
  /** the writer's page: found by their id (they may have been renamed since), else by the name the note shows */
  const writer = (m: { user: string; author: string }) => { const p = app.people.find((x) => x.id === m.user); return personHash(p ? nameOnRecipes(p) : m.author); };

  // a "today only" note disappears at midnight even if the page stays open
  let now = $state(Date.now());
  $effect(() => { const t = setInterval(() => (now = Date.now()), 60_000); return () => clearInterval(t); });
  const notes = $derived(board.messages.filter((m) => m.expires === null || m.expires * 1000 > now));

  let text = $state('');
  let keep = $state<'today' | 'date' | 'forever'>('today');
  let until = $state('');
  let busy = $state(false);
  let error = $state(''); // shown in the form: a toast would sit under the dialog

  const day = (t: number) => new Date(t * 1000).toDateString();
  function lasts(expires: number | null) {
    if (expires === null) return '';
    if (day(expires) === new Date(now).toDateString()) return 'Today only';
    return 'Until ' + new Date(expires * 1000).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
  }
  function when(t: number) {
    const d = new Date(t * 1000);
    return day(t) === new Date(now).toDateString() ? d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  function close() { board.compose = false; text = ''; keep = 'today'; until = ''; error = ''; }

  async function post(e: Event) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      const expires = keep === 'forever' ? null : endOfDay(keep === 'date' ? until : undefined);
      await api('POST', '/api/messages', { text, expires });
      await loadMessages();
      close();
    } catch (err: any) { error = err.message; } finally { busy = false; }
  }

  async function remove(id: string) {
    if (!(await confirmIt('Wipe this note off the whiteboard?', undefined, 'Remove', true))) return;
    try { await api('DELETE', `/api/messages/${id}`); await loadMessages(); } catch (err: any) { toast(err.message, 'error'); }
  }

  const focus = (el: HTMLElement) => { setTimeout(() => el.focus(), 0); };
</script>

{#if notes.length}
  <Section id="whiteboard" class="board" title="Whiteboard" count={notes.length}>
    <ul class="notes" aria-label="Kitchen whiteboard">
      {#each notes as m (m.id)}
        <li class="note">
          <p class="text">{@html renderInline(m.text)}</p>
          <div class="foot row">
            <span class="muted small"><a class="who" href={writer(m)}>{m.author}</a>{[when(m.created), lasts(m.expires)].filter(Boolean).map((t) => ' · ' + t).join('')}</span>
            <span class="spacer"></span>
            {#if app.session.user}
              <button class="btn ghost icon" aria-label="Remove note from {m.author}" title="Remove" onclick={() => remove(m.id)}><Icon name="x" size={16} /></button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  </Section>
{/if}

{#if board.compose}
  <dialog class="modal-host" aria-labelledby="board-title" use:modal={{ onclose: close, outside: true, active: tab.id === app.tab }}>
      <form class="modal card" onsubmit={post}>
        <h3 id="board-title">Write on the whiteboard</h3>
        <textarea class="field" rows="4" maxlength="500" bind:value={text} use:focus aria-label="Note" placeholder="We're out of eggs…"></textarea>
        <fieldset>
          <legend class="muted small">Keep it</legend>
          <label class="row"><input type="radio" bind:group={keep} value="today" /> Today only</label>
          <label class="row"><input type="radio" bind:group={keep} value="date" /> Until
            <input class="field" type="date" min={today()} bind:value={until} required={keep === 'date'} onfocus={() => (keep = 'date')} aria-label="Last day" />
          </label>
          <label class="row"><input type="radio" bind:group={keep} value="forever" /> Until someone removes it</label>
        </fieldset>
        {#if error}<p class="form-error" role="alert">{error}</p>{/if}
        <div class="row end">
          <button type="button" class="btn" onclick={close}>Cancel</button>
          <button class="btn primary" disabled={busy || !text.trim()}>Post</button>
        </div>
      </form>
  </dialog>
{/if}

<style>
  .notes { list-style: none; margin: 0; padding: 0; display: grid; gap: 12px; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); }
  .note { background: var(--p-yellow); color: var(--text); border-radius: var(--radius); padding: 12px 12px 4px 16px; display: grid; gap: 4px; align-content: space-between; }
  .text { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--serif); font-size: 17px; line-height: 1.4; padding-right: 4px; }
  .who { color: inherit; text-decoration: none; }
  .who:hover { color: var(--accent-ink); text-decoration: underline; }
  .foot { flex-wrap: nowrap; gap: 4px; min-height: 36px; }
  .modal { width: min(440px, calc(100vw - 32px)); padding: 20px; display: grid; gap: 12px; }
  .modal h3 { margin: 0; }
  textarea { resize: vertical; font: inherit; }
  fieldset { border: 0; margin: 0; padding: 0; display: grid; gap: 8px; }
  legend { padding: 0; margin-bottom: 4px; }
  fieldset label { gap: 8px; min-height: 36px; flex-wrap: nowrap; }
  fieldset input[type='date'] { width: auto; flex: 1; min-width: 0; }
  .end { justify-content: flex-end; }
</style>
