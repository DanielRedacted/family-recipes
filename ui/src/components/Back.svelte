<script lang="ts">
  import Icon from './Icon.svelte';
  import { app, back } from '../lib/state.svelte';
</script>

<!-- the window has no browser buttons: back to the previous page in this app (⌘[) -->
<button type="button" class="btn ghost icon back" aria-label="Back" title="Back (⌘[)" disabled={!app.canBack} onclick={back}>
  <Icon name="chevron-left" size={20} />
</button>

<style>
  .back { min-width: 32px; min-height: 32px; padding: 4px; flex: none; }
  .back:disabled { opacity: 0.3; }
</style>
