<script lang="ts">
  // A person's picture: their photo, else their emoji, else their initials on a pastel from their name.
  import { cardColor } from '../lib/icons';
  import { initials } from '../lib/people';

  let { name, picture, size = 40 }: { name: string; picture?: string | null; size?: number } = $props();
  const photo = $derived(picture?.startsWith('data:') ? picture : null);
</script>

<span class="av" style="--s:{size}px; background:{photo ? 'var(--sunken)' : cardColor(name)}" aria-hidden="true">
  {#if photo}<img src={photo} alt="" />
  {:else if picture}<span class="emoji">{picture}</span>
  {:else}{initials(name)}{/if}
</span>

<style>
  .av { flex: none; display: inline-grid; place-items: center; width: var(--s); height: var(--s); border-radius: 50%; overflow: hidden;
    color: var(--text); font: 600 calc(var(--s) * 0.38) / 1 var(--serif); letter-spacing: 0.02em; }
  img { width: 100%; height: 100%; object-fit: cover; }
  .emoji { font-size: calc(var(--s) * 0.55); line-height: 1; font-family: 'Apple Color Emoji', 'Segoe UI Emoji', 'Noto Color Emoji', sans-serif; }
</style>
