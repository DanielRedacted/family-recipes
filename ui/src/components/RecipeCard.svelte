<script lang="ts" module>
  import { app, nameOf, dirOf, favorites } from '../lib/state.svelte';
  import { fileUrl } from '../lib/api';
  import { vaultTarget } from '../lib/markdown';
  import { iconOf } from '../lib/icons';
  import type { Entry } from '../lib/types';

  export const folderIcon = (dir: string) => iconOf(nameOf(dir), app.folderMeta[dir]?.emoji);
  export const noteIcon = (f: Entry) => (f.kind === 'note' ? iconOf(f.title, f.frontmatter?.emoji, dirOf(f.path) ? folderIcon(dirOf(f.path)) : null) : null);
  export const isFav = (f: Entry) => favorites.paths.includes(f.path);
  export const badge = (f: Entry) => (f.kind === 'pdf' ? 'PDF' : f.kind === 'image' ? 'Image' : f.kind === 'other' ? nameOf(f.path).split('.').pop()?.toUpperCase() : f.recipe ? '' : 'Note');
  // the card's details line: how long it takes (not the batch size)
  const meta = (f: Entry) => String(f.frontmatter?.total_time ?? f.frontmatter?.cook_time ?? '');
  function cover(f: Entry): string | null {
    if (f.kind === 'image') return fileUrl(f.path);
    if (!f.cover) return null;
    if (/^https?:/i.test(f.cover)) return f.cover;
    const p = vaultTarget(f.cover, dirOf(f.path));
    return p ? fileUrl(p) : null;
  }
</script>

<script lang="ts">
  // A recipe (or any file) as a card: its picture or pastel, title, time and tags.
  import Icon from './Icon.svelte';
  import CardIcon from './CardIcon.svelte';
  import { noteHash } from '../lib/state.svelte';
  import { entryItems } from '../lib/ops';
  import { openMenu } from '../lib/menu.svelte';
  import { tagsOf } from '../lib/search';
  import { cardColor } from '../lib/icons';

  let { f }: { f: Entry } = $props();
  const img = $derived(cover(f));
  const ic = $derived(noteIcon(f));
  const menu = (e: MouseEvent) => openMenu(e, entryItems(f.path, false));
</script>

<div class="wrap">
  <a class="recipe card" class:photo={!!img} href={noteHash(f.path)} oncontextmenu={menu}>
    {#if img}
      <img src={img} alt="" loading="lazy" />
    {:else}
      <div class="ph" style="background:{cardColor(f.title, f.frontmatter?.color)}"><CardIcon icon={ic} size={44} /></div>
    {/if}
    <div class="info">
      <span class="rtitle">{#if img && ic}<CardIcon icon={ic} size={16} /> {/if}{f.title}{#if f.frontmatter?.favorite === true}<span class="star" title="Bookmarked" aria-label="Bookmarked"><Icon name="bookmark" size={14} /></span>{/if}{#if isFav(f)}<span class="star" title="Favorite" aria-label="Favorite"><Icon name="star" size={14} /></span>{/if}</span>
      {#if meta(f)}<span class="muted small">{meta(f)}</span>{/if}
      <span class="tags">
        {#if badge(f)}<span class="tag badge">{badge(f)}</span>{/if}
        {#each tagsOf(f).slice(0, 3) as t}<span class="tag">{t}</span>{/each}
        {#if f.frontmatter?.needs_review}<span class="tag warn">needs review</span>{/if}
      </span>
    </div>
  </a>
  <button class="more" aria-label="Actions for {f.title}" onclick={menu}>⋯</button>
</div>

<style>
  .wrap { position: relative; display: grid; }
  .more { position: absolute; top: 8px; right: 8px; border: 0; border-radius: 6px; background: var(--surface); color: var(--muted); padding: 0 8px; font-size: 16px; line-height: 24px; cursor: pointer; opacity: 0; box-shadow: var(--shadow); }
  .wrap:hover .more, .more:focus-visible { opacity: 1; }
  @media (hover: none) { .more { opacity: 0.85; } .info .rtitle { padding-right: 28px; } } /* the ⋯ button stays clear of the title */
  .star { display: inline-flex; vertical-align: -2px; color: var(--accent); margin-left: 4px; }
  /* the picture (or the card's colour with its icon) fills the card; title, details and tags sit on it */
  .recipe { position: relative; display: block; height: 186px; text-decoration: none; color: var(--text); overflow: hidden; }
  .recipe:hover { box-shadow: var(--shadow-md); border-color: var(--accent); }
  .recipe img, .ph { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  .ph { padding: 14px 16px; } /* the icon in the top-left corner, with room around it */
  .info { position: absolute; left: 0; right: 0; bottom: 0; padding: 8px 12px 12px; display: grid; gap: 4px; }
  /* every card the same height, and the overlay too: at most two title lines, one details line, one row of tags */
  .info .rtitle { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .info .muted { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .info .tags { flex-wrap: nowrap; overflow: hidden; }
  .info .tag { flex: none; }
  .recipe:not(.photo) .tag { background: color-mix(in srgb, var(--surface) 60%, transparent); }
  /* a photo: a dark fade behind white text */
  .photo .info { padding-top: 32px; color: #fff; background: linear-gradient(to top, rgba(0, 0, 0, 0.7), rgba(0, 0, 0, 0)); }
  .photo .info .muted { color: rgba(255, 255, 255, 0.85); }
  .photo .tag { background: rgba(255, 255, 255, 0.85); color: var(--text); }
  .rtitle { font-family: var(--serif); font-weight: 600; font-size: 16px; }
  .tags { display: flex; gap: 4px; flex-wrap: wrap; }
  .tag { font-size: 11px; padding: 2px 8px; border-radius: 10px; background: var(--sunken); color: var(--muted); }
  .tag.badge { background: var(--accent-soft); color: var(--accent-ink); }
  .tag.warn { background: var(--p-yellow); color: var(--text); }
</style>
