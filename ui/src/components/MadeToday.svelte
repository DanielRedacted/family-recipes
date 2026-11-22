<script lang="ts">
  import Icon from './Icon.svelte';
  import { app } from '../lib/state.svelte';
  import { madeToday } from '../lib/made';
  import { markMade } from '../lib/ops';
  import type { Entry } from '../lib/types';

  let { entry }: { entry: Entry } = $props();
  const done = $derived(madeToday(app.made, entry.path));
</script>

<p class="made no-print">
  <button class="btn" disabled={done} onclick={() => markMade(entry)}><Icon name="check" size={16} /> {done ? 'Made today ✓' : 'Made today'}</button>
</p>

<style>
  .made { margin: 32px 0 0; }
</style>
