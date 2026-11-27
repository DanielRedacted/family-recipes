<script lang="ts">
  import { setContext } from 'svelte';
  import { app, type Tab } from '../lib/state.svelte';
  import Folder from './Folder.svelte';
  import Note from './Note.svelte';
  import Settings from './Settings.svelte';
  import Import from './Import.svelte';
  import Ingredients from './Ingredients.svelte';
  import Calendar from './Calendar.svelte';
  import People from './People.svelte';
  import Charts from './Charts.svelte';

  /** One tab's page. Every tab's page stays alive (scroll, scaler, an unsaved edit); only the active one shows. */
  let { tab }: { tab: Tab } = $props();
  // svelte-ignore state_referenced_locally (a pane belongs to one tab for its whole life: keyed by its id)
  setContext('tab', tab);

  // every page opens at its top
  let el: HTMLElement;
  $effect(() => { tab.route; if (el) el.scrollTop = 0; });
</script>

<div class="pane" bind:this={el} inert={tab.id !== app.tab}>
  {#if tab.route.view === 'note'}
    {#key tab.route.path}<Note path={tab.route.path} mode={tab.route.mode} />{/key}
  {:else if tab.route.view === 'settings'}<Settings />
  {:else if tab.route.view === 'import'}<Import />
  {:else if tab.route.view === 'ingredients'}<Ingredients query={tab.route.path} />
  {:else if tab.route.view === 'calendar'}<Calendar />
  {:else if tab.route.view === 'charts'}<Charts />
  {:else if tab.route.view === 'people'}{#key tab.route.path}<People name={tab.route.path} />{/key}
  {:else}{#key tab.route.path}<Folder path={tab.route.path} />{/key}
  {/if}
</div>

<style>
  .pane { position: absolute; inset: 0; overflow-y: auto; min-width: 0; }
  /* laid out but unseen, so its scroll position holds */
  .pane[inert] { visibility: hidden; }
  @media print {
    .pane { position: static; overflow: visible; }
    .pane[inert] { display: none; }
  }
</style>
