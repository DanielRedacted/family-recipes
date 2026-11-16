<script lang="ts">
  import Icon from './Icon.svelte';
  import { getContext } from 'svelte';
  import { back, type Tab } from '../lib/state.svelte';
  const tab = getContext<Tab | undefined>('tab');
</script>

<!-- the window has no browser buttons: back to this tab's previous page (⌘[) -->
<button type="button" class="btn ghost icon back" aria-label="Back" title="Back (⌘[)" disabled={!tab || tab.pos === 0} onclick={back}>
  <Icon name="chevron-left" size={20} />
</button>

<style>
  .back { min-width: 32px; min-height: 32px; padding: 4px; flex: none; }
  .back:disabled { opacity: 0.3; }
</style>
