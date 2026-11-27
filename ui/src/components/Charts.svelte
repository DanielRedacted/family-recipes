<script lang="ts">
  import Back from './Back.svelte';
  import { volumeRows, weightRows, ovenRows, SAFE_TEMPS, SAFE_TEMPS_SOURCE, VOLUME_COLS, WEIGHT_COLS, CONVERT_UNITS, convert } from '../lib/charts';

  // every tab can have this page open: ids are per instance
  const id = $props.id();
  let amount = $state('1');
  let unit = $state('cup');
  const results = $derived(convert(amount, unit));
  const volume = volumeRows();
  const weight = weightRows();
  const oven = ovenRows();
</script>

<div class="page">
  <h1 class="title"><Back />Kitchen charts</h1>
  <p class="muted small">The same in every cookbook. The app uses these numbers when it scales a recipe or shows it in metric.</p>

  <section class="prose" aria-labelledby="{id}-conv">
    <h2 id="{id}-conv">Convert an amount</h2>
    <div class="conv">
      <div class="fieldset">
        <label for="{id}-amt">Amount</label>
        <input id="{id}-amt" class="field amt" bind:value={amount} autocomplete="off" autocorrect="off" spellcheck="false" enterkeyhint="done" aria-describedby="{id}-hint" />
      </div>
      <div class="fieldset">
        <label for="{id}-unit">Unit</label>
        <select id="{id}-unit" class="field" bind:value={unit}>
          {#each Object.entries(CONVERT_UNITS) as [kind, units]}
            <optgroup label={kind}>{#each units as u}<option value={u}>{u}</option>{/each}</optgroup>
          {/each}
        </select>
      </div>
    </div>
    <p id="{id}-hint" class="muted small">Fractions work too: 1 ½, ⅓ or 1/3.</p>
    <output for="{id}-amt {id}-unit" aria-live="polite">
      {#if results.length}
        <ul class="results">{#each results as r}<li>{#if r.about}<span class="muted about">about</span>{/if}<b>{r.value}</b> {r.unit}</li>{/each}</ul>
      {:else if amount.trim()}
        <p class="muted">Type a number, like 2 or 1 ½.</p>
      {/if}
    </output>
    <p class="muted small">Cups and spoons turn into weight only for a particular ingredient. To see recipes in grams, choose Metric in Settings → General.</p>
  </section>

  <section class="prose" aria-labelledby="{id}-vol">
    <h2 id="{id}-vol">Measures</h2>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrolling table must be reachable by keyboard to scroll it) -->
    <div class="table-wrap" tabindex="0" role="region" aria-labelledby="{id}-vol">
      <table>
        <thead><tr><th scope="col">Measure</th>{#each VOLUME_COLS as c}<th scope="col">{c}</th>{/each}</tr></thead>
        <tbody>{#each volume as r}<tr><th scope="row">{r.label}</th>{#each r.cells as v}<td>{v}</td>{/each}</tr>{/each}</tbody>
      </table>
    </div>
  </section>

  <div class="pair">
    <section class="prose" aria-labelledby="{id}-wt">
      <h2 id="{id}-wt">Weights</h2>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrolling table must be reachable by keyboard to scroll it) -->
    <div class="table-wrap" tabindex="0" role="region" aria-labelledby="{id}-wt">
        <table>
          <thead><tr><th scope="col">Weight</th>{#each WEIGHT_COLS as c}<th scope="col">{c}</th>{/each}</tr></thead>
          <tbody>{#each weight as r}<tr><th scope="row">{r.label}</th>{#each r.cells as v}<td>{v}</td>{/each}</tr>{/each}</tbody>
        </table>
      </div>
    </section>

    <section class="prose" aria-labelledby="{id}-oven">
      <h2 id="{id}-oven">Oven temperatures</h2>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrolling table must be reachable by keyboard to scroll it) -->
    <div class="table-wrap" tabindex="0" role="region" aria-labelledby="{id}-oven">
        <table>
          <thead><tr><th scope="col">°F</th><th scope="col">°C</th></tr></thead>
          <tbody>{#each oven as r}<tr><th scope="row">{r.f}°F</th><td>{r.c}°C</td></tr>{/each}</tbody>
        </table>
      </div>
    </section>
  </div>

  <section class="prose" aria-labelledby="{id}-safe">
    <h2 id="{id}-safe">Safe cooking temperatures</h2>
    <p class="muted small">The lowest temperature in the thickest part, measured with a food thermometer.</p>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrolling table must be reachable by keyboard to scroll it) -->
    <div class="table-wrap" tabindex="0" role="region" aria-labelledby="{id}-safe">
      <table>
        <thead><tr><th scope="col">Food</th><th scope="col">°F</th><th scope="col">°C</th><th scope="col">Then</th></tr></thead>
        <tbody>{#each SAFE_TEMPS as r}<tr><th scope="row">{r.food}</th><td>{r.f}°F</td><td>{r.c}°C</td><td>{r.rest ?? ''}</td></tr>{/each}</tbody>
      </table>
    </div>
    <p class="muted small">From the <a class="ext" href={SAFE_TEMPS_SOURCE}>USDA safe temperature chart</a>.</p>
  </section>
</div>

<style>
  .title { display: flex; align-items: center; gap: 4px; margin-left: -8px; }
  .page { padding: 16px 28px 80px; max-width: 820px; }
  h1 { margin: 8px 0 4px; }
  h2 { font-size: 1.15rem; margin: 28px 0 8px; }
  .conv { display: flex; flex-wrap: wrap; gap: 12px; }
  .fieldset { display: flex; flex-direction: column; gap: 4px; }
  .fieldset label { font-size: 13px; color: var(--muted); }
  .amt { width: 9em; }
  /* at least 16px, or iOS zooms in on focus */
  .conv .field { font-size: max(1rem, 1em); }
  .conv + p { margin: 6px 0 0; }
  output { display: block; min-height: 2.5em; }
  .results { list-style: none; padding: 0; margin: 12px 0 4px; display: flex; flex-wrap: wrap; gap: 6px 20px; }
  .results li { font: 15px var(--sans); white-space: nowrap; }
  .about { margin-right: 0.3em; }
  /* notes stay in the app's sans; .prose is for the tables */
  .page p { font-family: var(--sans); }
  .results b { font-family: var(--serif); font-size: 1.35rem; font-weight: 600; }
  .prose .table-wrap { margin: 0.5em 0; }
  /* numbers and units never break (".prose" breaks anywhere, for recipe text); a narrow screen scrolls the table instead */
  .prose table { overflow-wrap: normal; }
  .prose td, .prose thead th { white-space: nowrap; font-variant-numeric: tabular-nums; }
  .prose tbody th { font-weight: 500; }
  .table-wrap:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .pair { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 0 28px; }
  @media (max-width: 899px) {
    .page { padding: 8px 16px 96px; }
    .prose th, .prose td { padding: 6px 8px; }
  }
</style>
