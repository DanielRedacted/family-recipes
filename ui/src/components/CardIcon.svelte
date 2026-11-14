<script lang="ts">
  import Icon from './Icon.svelte';
  import type { CardIcon } from '../lib/icons';
  let { icon, size = 20 }: { icon: CardIcon; size?: number } = $props();
</script>

{#if icon && 'emoji' in icon}<span class="emoji" style="font-size:{size}px" aria-hidden="true">{icon.emoji}</span>
{:else if icon}<span class="lucide" aria-hidden="true"><Icon name={icon.icon} {size} /></span>{/if}

<style>
  .emoji { line-height: 1; font-family: 'Apple Color Emoji', 'Segoe UI Emoji', 'Noto Color Emoji', sans-serif; }
  .lucide { display: inline-flex; color: var(--muted); }
</style>
