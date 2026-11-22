<script lang="ts">
  import { app } from '../lib/state.svelte';
  import { helpPages, searchHelp, type HelpPage } from '../lib/help';
  import { openFeedback } from './Feedback.svelte';
  import { renderMarkdown } from '../lib/markdown';

  // #/settings/help lists the pages; #/settings/help/<slug> shows one
  const admin = $derived(app.session.role === 'admin');
  const all = $derived(helpPages(admin));
  const slug = $derived(app.route.path.split('/')[1] ?? '');
  const page = $derived(all.find((p) => p.slug === slug));
  const html = $derived(page ? renderMarkdown(page.body, '') : '');

  let q = $state('');
  const results = $derived(q.trim() ? searchHelp(q.trim(), admin) : null);
  const groups = $derived([
    { label: 'Using the app', items: all.filter((p) => p.audience === 'everyone') },
    { label: 'For admins', items: all.filter((p) => p.audience === 'admin') },
  ].filter((g) => g.items.length));

  let top = $state<HTMLElement>();
  // opening a page (from the list or a search result) starts at its top with the search cleared
  $effect(() => { slug; q = ''; top?.scrollIntoView({ block: 'start' }); });

  const href = (p: HelpPage) => `#/settings/help/${p.slug}`;
</script>

<div class="page" class:reading={!!page} bind:this={top}>
  <aside class="toc" aria-label="Help pages">
    {#if !page}<h1>Help</h1>{/if}
    <label class="search">
      <span class="sr-only">Search help</span>
      <input class="field" type="search" bind:value={q} placeholder="Search help" autocomplete="off" />
    </label>
    {#if results}
      <p class="muted small" role="status">{results.length ? `${results.length} ${results.length === 1 ? 'page' : 'pages'}` : 'No page mentions that. Try another word.'}</p>
      <ul>
        {#each results as p (p.slug)}
          <li class:on={p.slug === slug}><a href={href(p)} aria-current={p.slug === slug ? 'page' : undefined}>{p.title}</a>
            {#if p.summary}<span class="muted small">{p.summary}</span>{/if}</li>
        {/each}
      </ul>
    {:else}
      {#each groups as g (g.label)}
        <h2>{g.label}</h2>
        <ul>
          {#each g.items as p (p.slug)}
            <li class:on={p.slug === slug}><a href={href(p)} aria-current={p.slug === slug ? 'page' : undefined}>{p.title}</a>
              {#if !page && p.summary}<span class="muted small">{p.summary}</span>{/if}</li>
          {/each}
        </ul>
      {/each}
    {/if}
    <p class="muted small fb">Something not working, or an idea? <button class="linkish" onclick={openFeedback}>Send feedback</button> (or search “feedback” from anywhere).</p>
  </aside>

  {#if page}
    <article>
      <a class="back" href="#/settings/help">‹ All help pages</a>
      <div class="prose"><h1>{page.title}</h1>{@html html}</div>
    </article>
  {:else if slug}
    <article><p>There's no help page called “{slug}”. Pick one from the list.</p></article>
  {/if}
</div>

<style>
  .page { padding: 16px 28px 80px; display: grid; grid-template-columns: minmax(0, 1fr); gap: 28px; max-width: 760px; }
  .page.reading { grid-template-columns: 220px minmax(0, 1fr); max-width: 1040px; align-items: start; }
  .toc { display: grid; gap: 8px; align-content: start; }
  .reading .toc { position: sticky; top: 16px; max-height: calc(100vh - 32px); overflow-y: auto; }
  .toc h2 { margin: 12px 0 0; font: 600 14px var(--sans); color: var(--muted); }
  .toc ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .toc li { display: grid; gap: 2px; padding: 4px 8px; border-radius: 6px; }
  .toc li.on { background: var(--accent-soft); }
  .toc a { color: var(--text); text-decoration: none; }
  .toc a[aria-current='page'] { font-weight: 600; }
  .toc a:hover { text-decoration: underline; }
  .reading .toc li { padding-block: 2px; }
  .search .field { width: 100%; }
  .toc h1 { margin: 8px 0 4px; }
  .fb { margin: 16px 0 0; }
  .linkish { border: 0; background: none; padding: 0; font: inherit; color: var(--accent-ink); text-decoration: underline; cursor: pointer; }
  /* table cells break only words that can't fit, so "Reader" stays whole */
  article :global(:is(td, th)) { overflow-wrap: break-word; }
  .back { display: none; color: var(--muted); text-decoration: none; font-size: 14px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  @media (max-width: 899px) {
    .page { padding: 8px 16px 96px; }
    .page.reading { grid-template-columns: minmax(0, 1fr); }
    .reading .toc { display: none; }
    .back { display: inline-block; margin-bottom: 8px; }
  }
</style>
