<script lang="ts">
  // People (#/people) and one person's page (#/people/<name>): everyone the recipes credit, with or
  // without an account, and what they gave the book. Credits come from the recipes (lib/people.ts),
  // so a page outlives its account; favorites and the picture belong to people who sign in.
  import { getContext } from 'svelte';
  import Back from './Back.svelte';
  import Avatar from './Avatar.svelte';
  import Section from './Section.svelte';
  import RecipeCard from './RecipeCard.svelte';
  import { app, loadPeople, groupTitle, noteHash, type Tab } from '../lib/state.svelte';
  import { renderInline } from '../lib/markdown';
  import { api } from '../lib/api';
  import { modal } from '../lib/modal';
  import { suggestEmoji } from '../lib/icons';
  import { creditsOf, directory, findPerson, namesOf, nameOnRecipes, notesBy, personHash, type Listed } from '../lib/people';
  import type { Entry } from '../lib/types';

  let { name }: { name: string } = $props();
  const tab = getContext<Tab>('tab');

  const byTitle = (a: Entry, b: Entry) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: 'base' });
  const dir = $derived(name ? null : directory(app.files, app.people));
  const person = $derived(name ? findPerson(app.people, name) : undefined);
  const shown = $derived(person ? nameOnRecipes(person) : name);
  const credits = $derived(name ? creditsOf(app.files, person ? namesOf(person) : [name]) : null);
  const favorites = $derived((person?.favorites ?? []).map((p) => app.byPath[p]).filter((f): f is Entry => f?.kind === 'note').sort(byTitle));
  const total = $derived(credits ? credits.wrote.length + credits.added.length + credits.edited.length : 0);
  /** the notes they left on recipes: their own credit, apart from the recipes they wrote, added or edited */
  const notes = $derived(name ? notesBy(app.files, person ? namesOf(person) : [name]) : []);
  const noteDay = (d: string) => { const t = new Date(d + 'T12:00'); return isNaN(+t) ? d : t.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' }); };
  const me = $derived(!!person && person.id === app.session.user?.id);
  const canPicture = $derived(!!person && (me || app.session.role === 'admin'));
  const since = (t?: number) => (t ? new Date(t * 1000).toLocaleDateString(undefined, { month: 'long', year: 'numeric' }) : '');
  const recipesLine = (n: number) => `${n} recipe${n === 1 ? '' : 's'}`;

  // ---------- the picture: a photo (cropped square on this device) or an emoji ----------
  let picking = $state(false);
  let busy = $state(false);
  let error = $state(''); // shown in the dialog: a toast would sit under it
  const emoji = $derived(suggestEmoji(shown, 30));

  async function setPicture(picture: string | null) {
    if (!person) return;
    busy = true;
    error = '';
    try {
      await api('PUT', `/api/users/${encodeURIComponent(person.id)}/picture`, { picture });
      await loadPeople();
      picking = false;
    } catch (e: any) { error = e.message; } finally { busy = false; }
  }

  /** The middle square of a photo at 256px, as a JPEG data URL (small enough to keep with the account). */
  async function squared(file: File): Promise<string> {
    const url = URL.createObjectURL(file);
    try {
      const img = new Image();
      img.src = url;
      await img.decode();
      const side = Math.min(img.naturalWidth, img.naturalHeight);
      const c = document.createElement('canvas');
      c.width = c.height = 256;
      c.getContext('2d')!.drawImage(img, (img.naturalWidth - side) / 2, (img.naturalHeight - side) / 2, side, side, 0, 0, 256, 256);
      return c.toDataURL('image/jpeg', 0.85);
    } finally { URL.revokeObjectURL(url); }
  }

  async function chosePhoto(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try { await setPicture(await squared(file)); } catch { error = "That picture can't be opened. Try a JPEG or PNG."; }
  }
</script>

{#snippet tile(l: Listed)}
  <a class="person card" href={personHash(l.name)}>
    <Avatar name={l.name} picture={l.person?.picture} size={44} />
    <span class="who">
      <span class="pname">{l.name}</span>
      <span class="muted small">{l.recipes ? recipesLine(l.recipes) : 'No recipes yet'}{#if l.person?.favorites?.length}, {l.person.favorites.length} favorite{l.person.favorites.length === 1 ? '' : 's'}{/if}</span>
    </span>
  </a>
{/snippet}

{#snippet cards(list: Entry[])}
  <div class="card-grid">{#each [...list].sort(byTitle) as f (f.path)}<RecipeCard {f} />{/each}</div>
{/snippet}

<div class="page">
  {#if dir}
    <h1 class="title"><Back />People</h1>
    <p class="muted lede">Everyone who has written, added or changed a recipe in the book.</p>
    {#if dir.family.length}
      <Section id="people-family" title={groupTitle()} count={dir.family.length}>
        <div class="people">{#each dir.family as l (l.person?.id)}{@render tile(l)}{/each}</div>
      </Section>
    {/if}
    {#if dir.others.length}
      <Section id="people-others" title="Also in the book" count={dir.others.length}>
        <div class="people">{#each dir.others as l (l.name)}{@render tile(l)}{/each}</div>
      </Section>
    {/if}
    {#if !dir.family.length && !dir.others.length && app.loaded}<p class="muted">Nobody is credited yet. Names appear here once someone adds or writes a recipe.</p>{/if}
  {:else}
    <div class="crumbs"><Back /><a class="small" href="#/people">People</a></div>
    <header class="hero">
      <Avatar name={shown} picture={person?.picture} size={88} />
      <div class="who">
        <h1>{shown}</h1>
        {#if person}
          <p class="muted">{person.name !== shown ? `Signs in as ${person.name}` : 'Signs in'}{#if person.created}, since {since(person.created)}{/if}</p>
        {:else if total}
          <p class="muted">Credited in the book's recipes</p>
        {:else if notes.length}
          <p class="muted">Left notes on the book's recipes</p>
        {/if}
        {#if canPicture}<button class="btn" onclick={() => { error = ''; picking = true; }}>{person?.picture ? 'Change picture…' : 'Add a picture…'}</button>{/if}
      </div>
    </header>

    {#if credits && total}
      {#if credits.wrote.length}<Section id="person-wrote" title="Recipes" count={credits.wrote.length}>{@render cards(credits.wrote)}</Section>{/if}
      {#if credits.added.length}<Section id="person-added" title="Added to the book" count={credits.added.length}>{@render cards(credits.added)}</Section>{/if}
      {#if credits.edited.length}<Section id="person-edited" title="Edited" count={credits.edited.length}>{@render cards(credits.edited)}</Section>{/if}
    {/if}
    {#if notes.length}
      <Section id="person-notes" title="Notes" count={notes.length}>
        <ul class="pnotes">
          {#each notes as n (n.entry.path + n.note.id)}
            <li class="pnote">
              <a class="on" href={noteHash(n.entry.path) + '?note=' + encodeURIComponent(n.note.id)}>{n.entry.title}</a>
              {#if n.note.quote}<p class="quote">“{n.note.quote}”</p>{/if}
              <p class="text">{@html renderInline(n.note.text)}</p>
              <p class="muted small">{noteDay(n.note.date)}</p>
            </li>
          {/each}
        </ul>
      </Section>
    {/if}
    {#if favorites.length}<Section id="person-favorites" title="Favorites" count={favorites.length}>{@render cards(favorites)}</Section>{/if}

    {#if app.loaded && !person && !total && !notes.length}
      <p class="muted">No recipe in the book credits “{name}”. <a href="#/people">See everyone</a></p>
    {:else if app.loaded && !total && !favorites.length && !notes.length}
      <p class="muted">{me ? 'Recipes you write, add or edit show here, and so do your favorites.' : `Nothing from ${person?.name ?? shown} yet.`}</p>
    {/if}
  {/if}
</div>

{#if picking && person}
  <dialog class="modal-host" aria-labelledby="pic-title" use:modal={{ onclose: () => (picking = false), outside: true, active: tab.id === app.tab }}>
    <div class="modal card">
      <h3 id="pic-title">{me ? 'Your picture' : `${person.name}'s picture`}</h3>
      <div class="row">
        <Avatar name={shown} picture={person.picture} size={56} />
        <label class="btn">Choose a photo…<input class="vh" type="file" accept="image/*" onchange={chosePhoto} disabled={busy} /></label>
        {#if person.picture}<button class="btn" onclick={() => setPicture(null)} disabled={busy}>Remove</button>{/if}
      </div>
      <fieldset>
        <legend>Or an emoji</legend>
        <div class="emos">
          {#each emoji as e}<button class="emo" class:on={person.picture === e} aria-label="Use {e}" onclick={() => setPicture(e)} disabled={busy}>{e}</button>{/each}
        </div>
      </fieldset>
      {#if error}<p class="form-error" role="alert">{error}</p>{/if}
      <div class="row end"><button class="btn" onclick={() => (picking = false)}>Done</button></div>
    </div>
  </dialog>
{/if}

<style>
  .page { padding: 16px 28px 80px; max-width: 1200px; }
  .title { display: flex; align-items: center; gap: 4px; margin: 8px 0 4px -8px; }
  .lede { margin: 0 0 8px; }
  .people { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; }
  .person { display: flex; align-items: center; gap: 12px; padding: 12px 16px; text-decoration: none; color: var(--text); min-height: 68px; }
  .person:hover { border-color: var(--accent); }
  .who { display: grid; gap: 2px; min-width: 0; justify-items: start; }
  .pname { font: 600 17px/1.3 var(--serif); overflow-wrap: anywhere; }
  .crumbs { display: flex; align-items: center; gap: 4px; margin: 0 0 8px -8px; }
  .crumbs a { color: var(--accent-ink); text-decoration: none; }
  .crumbs a:hover { text-decoration: underline; }
  /* the person: their picture and their name, as large as a recipe's title */
  .hero { display: flex; align-items: center; gap: 20px; margin: 8px 0 8px; }
  .hero h1 { margin: 0; font-size: 2rem; line-height: 1.25; overflow-wrap: anywhere; }
  .hero p { margin: 0 0 8px; }
  .modal { width: min(440px, calc(100vw - 32px)); padding: 20px; display: grid; gap: 16px; max-height: 90vh; overflow: auto; }
  .modal h3 { margin: 0; }
  .modal .row { gap: 12px; }
  label.btn { position: relative; cursor: pointer; }
  label.btn:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
  .vh { position: absolute; width: 1px; height: 1px; opacity: 0; pointer-events: none; }
  fieldset { border: 0; padding: 0; margin: 0; }
  legend { font-weight: 600; margin-bottom: 8px; font-size: 14px; }
  .emos { display: grid; grid-template-columns: repeat(auto-fill, minmax(40px, 1fr)); gap: 4px; }
  .emo { font-size: 22px; background: none; border: 1px solid transparent; border-radius: 8px; padding: 4px; min-height: 40px; cursor: pointer; }
  .emo:hover { background: var(--sunken); }
  .emo.on { border-color: var(--accent); background: var(--accent-soft); }
  .end { justify-content: flex-end; }
  /* a note they left: the recipe it's on, the words it's about, the note (as in the recipe's Notes list) */
  .pnotes { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .pnote { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 14px; display: grid; gap: 4px; align-content: start; }
  .pnote .on { font: 600 16px/1.3 var(--serif); color: var(--text); text-decoration: none; }
  .pnote .on:hover { color: var(--accent-ink); text-decoration: underline; }
  .pnote p { margin: 0; }
  .pnote .quote { font: italic 14px/1.4 var(--serif); color: var(--muted); overflow-wrap: anywhere;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .pnote .text { font: 16px/1.4 var(--serif); white-space: pre-wrap; overflow-wrap: anywhere; }
  @media (max-width: 899px) {
    .page { padding: 8px 16px 96px; }
    .hero { gap: 16px; }
    .hero h1 { font-size: 1.6rem; }
  }
</style>
