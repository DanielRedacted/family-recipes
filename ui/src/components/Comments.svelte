<script lang="ts">
  // Notes on a recipe, Google Docs style: select words in the reader → Add note. Everyone sees them; people who
  // can edit add, change and delete any of them. They live in the recipe's `comments` property (lib/comments.ts).
  import Icon from './Icon.svelte';
  import { getContext, tick, untrack } from 'svelte';
  import { app, canWrite, toast, type Tab } from '../lib/state.svelte';
  import { commentsOf, addComment, editComment, removeComment, newId, markComments, clearMarks, selectionIn, type Comment } from '../lib/comments';
  import { saveComments } from '../lib/ops';
  import { today } from '../lib/credit';
  import { confirmIt } from '../lib/dialog.svelte';
  import { modal } from '../lib/modal';
  import { personHash } from '../lib/people';
  import { renderInline } from '../lib/markdown';
  import type { Entry } from '../lib/types';

  /** root: the reader's .prose; html: what it shows, so the highlights follow every re-render */
  let { entry, root, html }: { entry: Entry; root: HTMLElement | undefined; html: string } = $props();
  const tab = getContext<Tab>('tab');
  const comments = $derived(commentsOf(entry.frontmatter));

  // shown or hidden on this device, for every recipe
  let shown = $state((() => { try { return localStorage.getItem('notes') !== 'off'; } catch { return true; } })());
  $effect(() => { try { localStorage.setItem('notes', shown ? 'on' : 'off'); } catch {} });

  let current = $state<string | null>(null);
  /** where each note's words start in the page; a note missing here wasn't found */
  let at = $state(new Map<string, number>());

  // the highlights, again after each render: scaling, units, a portion or a version replace the page
  $effect(() => {
    html;
    const list = shown ? comments : [];
    const cur = current;
    if (!root) return;
    at = markComments(root, list, cur);
  });
  $effect(() => () => { if (root) clearMarks(root); });

  // in reading order: notes on the whole recipe, then by where their words are, then those not found
  const ordered = $derived([...comments].sort((a, b) => rank(a) - rank(b)));
  const rank = (c: Comment) => (!c.quote ? -1 : at.get(c.id) ?? Number.MAX_SAFE_INTEGER);

  const motion = (): ScrollBehavior => (matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth');
  const cardId = (id: string) => `cnote-${tab.id}-${id}`;

  // a link to one note (from a person's page, `#/n/<path>?note=<id>`) opens the recipe at it
  let opened = ''; // once per link, not again whenever the notes change
  $effect(() => {
    const hash = tab.route.hash ?? '';
    const id = new URLSearchParams(hash.split('?')[1] ?? '').get('note');
    if (!id || !root || opened === hash || !comments.some((c) => c.id === id)) return;
    opened = hash;
    untrack(() => { current = id; shown = true; });
    tick().then(() => (root?.querySelector(`mark[data-id="${CSS.escape(id)}"]`) ?? document.getElementById(cardId(id)))?.scrollIntoView({ block: 'center' }));
  });

  // a highlight opens its note
  $effect(() => {
    const r = root;
    if (!r) return;
    const click = async (e: MouseEvent) => {
      const m = (e.target as Element).closest?.('mark.cmark') as HTMLElement | null;
      if (!m || getSelection()?.isCollapsed === false) return;
      current = m.dataset.id ?? null;
      await tick();
      document.getElementById(cardId(current!))?.scrollIntoView({ block: 'nearest', behavior: motion() });
    };
    r.addEventListener('click', click);
    return () => r.removeEventListener('click', click);
  });
  // and a note shows its words
  function show(c: Comment) {
    current = c.id;
    tick().then(() => root?.querySelector(`mark[data-id="${c.id}"]`)?.scrollIntoView({ block: 'center', behavior: motion() }));
  }

  // ---------- the Add note button by a selection ----------
  let pick = $state<{ quote: string; nth: number; x: number; y: number } | null>(null);
  $effect(() => {
    const r = root;
    if (!r || !canWrite() || tab.id !== app.tab) { pick = null; return; }
    let t: ReturnType<typeof setTimeout>;
    const look = () => {
      clearTimeout(t);
      t = setTimeout(() => {
        const s = selectionIn(r);
        // below the words, clear of the phone's own copy menu above them
        pick = s && s.rect.bottom > 0 && s.rect.top < innerHeight
          ? { quote: s.quote, nth: s.nth, x: Math.min(Math.max(8, s.rect.left), innerWidth - 136), y: Math.min(s.rect.bottom + 8, innerHeight - 56) }
          : null;
      }, 150);
    };
    document.addEventListener('selectionchange', look);
    addEventListener('scroll', look, true);
    return () => { clearTimeout(t); document.removeEventListener('selectionchange', look); removeEventListener('scroll', look, true); };
  });

  // ---------- writing, changing and deleting ----------
  let compose = $state<{ quote: string; nth: number } | null>(null);
  let text = $state('');
  let busy = $state(false);
  let error = $state(''); // shown in the form: a toast would sit under the dialog
  let editing = $state<string | null>(null);
  let draft = $state('');

  /** who the note is from, credited on the note alone (never as one of the recipe's authors or editors): the
   *  full name, as on recipes; on the Mac, the "Editing as" person, else "Admin" */
  const author = () => { const u = app.session.user; return u ? (u.id !== 'admin' && u.full_name) || u.name : ''; };

  function start(quote = '', nth = 0) { compose = { quote, nth }; text = ''; error = ''; pick = null; shown = true; }
  const close = () => { compose = null; };

  async function add(e: Event) {
    e.preventDefault();
    if (!compose || !text.trim()) return;
    busy = true;
    error = '';
    try {
      const c: Comment = { id: newId(), quote: compose.quote, nth: compose.nth, text: text.trim(), by: author(), date: today() };
      await saveComments(entry.path, (raw) => addComment(raw, c));
      getSelection()?.removeAllRanges();
      current = c.id;
      close();
    } catch (err: any) { error = err.message; } finally { busy = false; }
  }

  async function saveEdit(e: Event, c: Comment) {
    e.preventDefault();
    if (!draft.trim()) return;
    busy = true;
    try {
      const by = author();
      await saveComments(entry.path, (raw) => editComment(raw, c.id, draft.trim(), by, today()));
      editing = null;
    } catch (err: any) { toast(err.message, 'error'); } finally { busy = false; }
  }

  async function remove(c: Comment) {
    if (!(await confirmIt('Delete this note?', c.text.length > 80 ? c.text.slice(0, 80) + '…' : c.text, 'Delete', true))) return;
    try { await saveComments(entry.path, (raw) => removeComment(raw, c.id)); } catch (err: any) { toast(err.message, 'error'); }
  }

  const day = (d: string) => { const t = new Date(d + 'T12:00'); return isNaN(+t) ? d : t.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: t.getFullYear() === new Date().getFullYear() ? undefined : 'numeric' }); };
  const focus = (el: HTMLElement) => { setTimeout(() => el.focus(), 0); };
</script>

{#if comments.length || canWrite()}
  <aside class="comments no-print" aria-labelledby="notes-{tab.id}">
    <div class="head row">
      <h2 id="notes-{tab.id}">Notes{#if comments.length}<span class="count">{comments.length}</span>{/if}</h2>
      <span class="spacer"></span>
      {#if comments.length}
        <button class="btn ghost small" aria-expanded={shown} aria-controls="notes-list-{tab.id}" onclick={() => (shown = !shown)}>{shown ? 'Hide' : 'Show'}</button>
      {/if}
      {#if canWrite()}<button class="btn small" onclick={() => start()}><Icon name="plus" size={14} /> Add note</button>{/if}
    </div>
    {#if shown}
      {#if canWrite() && !comments.length}<p class="muted small hint">Select words in the recipe to leave a note on them.</p>{/if}
      <ul id="notes-list-{tab.id}">
        {#each ordered as c (c.id)}
          <li id={cardId(c.id)} class="cnote" class:on={current === c.id}>
            {#if c.quote}
              {#if at.has(c.id)}
                <button class="quote" onclick={() => show(c)} title="Show in the recipe">“{c.quote}”</button>
              {:else}
                <p class="quote gone">“{c.quote}” <span class="small">Not found in the recipe as shown</span></p>
              {/if}
            {/if}
            {#if editing === c.id}
              <form class="edit" onsubmit={(e) => saveEdit(e, c)}>
                <textarea class="field" rows="3" maxlength="1000" bind:value={draft} use:focus aria-label="Note"></textarea>
                <div class="row end">
                  <button type="button" class="btn small" onclick={() => (editing = null)}>Cancel</button>
                  <button class="btn primary small" disabled={busy || !draft.trim()}>Save</button>
                </div>
              </form>
            {:else}
              <p class="text">{@html renderInline(c.text)}</p>
            {/if}
            <div class="foot row">
              <span class="muted small">
                {#if c.by}<a class="who" href={personHash(c.by)}>{c.by}</a>{' · '}{/if}{day(c.date)}{#if c.edited_by}, edited by {c.edited_by}{/if}
              </span>
              <span class="spacer"></span>
              {#if canWrite() && editing !== c.id}
                <button class="btn ghost icon" aria-label="Edit note from {c.by || 'someone'}" title="Edit" onclick={() => { editing = c.id; draft = c.text; }}><Icon name="pencil" size={16} /></button>
                <button class="btn ghost icon" aria-label="Delete note from {c.by || 'someone'}" title="Delete" onclick={() => remove(c)}><Icon name="trash" size={16} /></button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </aside>
{/if}

{#if pick}
  <!-- mousedown would clear the selection before the click -->
  <button class="btn primary addnote no-print" style="left:{pick.x}px; top:{pick.y}px" onmousedown={(e) => e.preventDefault()} onclick={() => start(pick!.quote, pick!.nth)}>
    <Icon name="message" size={16} /> Add note
  </button>
{/if}

{#if compose}
  <dialog class="modal-host" aria-labelledby="note-title-{tab.id}" use:modal={{ onclose: close, outside: true, active: tab.id === app.tab }}>
    <form class="modal card" onsubmit={add}>
      <h3 id="note-title-{tab.id}">{compose.quote ? 'Add a note' : 'Add a note on the whole recipe'}</h3>
      {#if compose.quote}<p class="quote">“{compose.quote}”</p>{/if}
      <textarea class="field" rows="4" maxlength="1000" bind:value={text} use:focus aria-label="Note" placeholder="Bread flour works better here…"></textarea>
      {#if error}<p class="form-error" role="alert">{error}</p>{/if}
      <div class="row end">
        <button type="button" class="btn" onclick={close}>Cancel</button>
        <button class="btn primary" disabled={busy || !text.trim()}>Save</button>
      </div>
    </form>
  </dialog>
{/if}

<style>
  /* the highlighted words, in the reader's .prose; an underline as well as the wash, so they show without colour */
  :global(mark.cmark) { background: var(--accent-soft); color: inherit; border-bottom: 2px solid var(--accent); border-radius: 2px; cursor: pointer; }
  :global(mark.cmark.on) { background: var(--accent); color: var(--on-accent); }
  :global(mark.cmark.on a) { color: inherit; }
  @media (forced-colors: active) { :global(mark.cmark) { background: Mark; color: MarkText; } }
  @media print { :global(mark.cmark) { background: none; border: 0; } }

  .comments { margin: 32px 0 0; padding-top: 12px; border-top: 1px solid var(--border); max-width: 760px; display: grid; gap: 12px; align-content: start; }
  .head { gap: 8px; }
  h2 { font: 600 14px var(--sans); color: var(--muted); margin: 0; }
  .count { margin-left: 6px; font-weight: 500; }
  .hint { margin: 0; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  .cnote { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 10px 8px 2px 12px; display: grid; gap: 4px; }
  .cnote.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .quote { margin: 0; padding: 0; border: 0; background: none; text-align: left; font: italic 14px/1.4 var(--serif); color: var(--muted); cursor: pointer; overflow-wrap: anywhere;
    display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  button.quote:hover { color: var(--accent-ink); }
  .quote.gone { cursor: default; display: block; }
  .quote.gone .small { font-style: normal; display: block; }
  .text { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--serif); font-size: 16px; line-height: 1.4; padding-right: 4px; }
  .foot { flex-wrap: nowrap; gap: 2px; min-height: 36px; }
  .who { color: inherit; text-decoration: none; }
  .who:hover { color: var(--accent-ink); text-decoration: underline; }
  .edit { display: grid; gap: 8px; padding-right: 4px; }
  textarea { resize: vertical; font: inherit; }
  .end { justify-content: flex-end; }
  .addnote { position: fixed; z-index: 20; min-height: 36px; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18); }
  .modal { width: min(440px, calc(100vw - 32px)); padding: 20px; display: grid; gap: 12px; }
  .modal h3 { margin: 0; }
  .modal .quote { cursor: default; -webkit-line-clamp: 4; line-clamp: 4; }
</style>
