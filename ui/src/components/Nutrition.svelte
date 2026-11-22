<script lang="ts">
  // A Nutrition Facts label in the FDA's layout (21 CFR 101.9, 2016 format). The reader always shows
  // the estimate worked out from the ingredients (lib/nutrition.ts) at its scale and portion, even when
  // the recipe has `nutrition:` values; those are shown as written only on the importer's Preview.
  import { DAILY, NUTRIENTS, UNIT, dailyValue, estimate, labelAmount, type Nutrient } from '../lib/nutrition';
  import { foods, loadFoods } from '../lib/ingredients.svelte';
  import { subRecipeOf } from '../lib/markdown';
  import type { Yield } from '../lib/scale';
  import type { Entry } from '../lib/types';

  let { data = null, entry = null, factor = 1, portion = 0, yields = null }: {
    /** the `nutrition:` property (importer Preview only) */
    data?: unknown;
    /** the recipe to estimate */
    entry?: Entry | null;
    factor?: number;
    portion?: number;
    /** what the recipe makes (servings, else yield) */
    yields?: Yield | null;
  } = $props();
  const id = $props.id();

  const LABELS: Record<Nutrient, string> = {
    calories: 'Calories', fat: 'Total Fat', saturated_fat: 'Saturated Fat', trans_fat: 'Trans Fat', cholesterol: 'Cholesterol',
    sodium: 'Sodium', carbohydrates: 'Total Carbohydrate', fiber: 'Dietary Fiber', sugar: 'Total Sugars', protein: 'Protein',
    vitamin_d: 'Vitamin D', calcium: 'Calcium', iron: 'Iron', potassium: 'Potassium',
  };
  const SUB = new Set<string>(['saturated_fat', 'trans_fat', 'fiber', 'sugar', 'polyunsaturated_fat', 'monounsaturated_fat', 'unsaturated_fat']);
  const MAIN: Nutrient[] = ['fat', 'saturated_fat', 'trans_fat', 'cholesterol', 'sodium', 'carbohydrates', 'fiber', 'sugar', 'protein'];
  const VITAMINS: Nutrient[] = ['vitamin_d', 'calcium', 'iron', 'potassium'];
  const human = (k: string) => k.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase());

  // ---- the recipe's own values, as written
  const own = $derived.by(() => {
    if (!data || typeof data !== 'object' || Array.isArray(data)) return null;
    const m = Object.fromEntries(Object.entries(data as Record<string, unknown>)
      .filter(([, v]) => v !== null && v !== '' && typeof v !== 'object').map(([k, v]) => [k, String(v)]));
    const { serving_size, ...rest } = m;
    return Object.keys(rest).length ? { serving: serving_size ?? null, values: rest } : null;
  });

  // ---- otherwise worked out from the ingredients
  $effect(() => { if (entry && !own) loadFoods(); });
  const est = $derived(entry && !own && foods.loaded ? estimate(entry, foods.table, subRecipeOf, factor, portion) : null);
  /** "split into" for a recipe that doesn't say what it makes; not saved */
  let split = $state(1);
  const servings = $derived(yields ? yields.n * factor : Math.max(1, Math.round(split) || 1));
  const perServing = $derived(!!yields || servings > 1);
  const singular = (u: string) => (/^(servings?|people|persons?|)$/i.test(u) ? 'serving' : u.replace(/ves$/, 'f').replace(/s$/, ''));
  const servingSize = $derived(yields ? `1 ${singular(yields.unit)}` : servings > 1 ? `1/${servings} of the recipe` : 'The whole recipe');
  const plural = (n: number) => (yields && !/^(servings?|people|persons?|)$/i.test(yields.unit) && n !== 1 ? `${singular(yields.unit)}s` : n === 1 ? 'serving' : 'servings');

  type Row = { key: string; label: string; amount: string; dv: number | null; sub: boolean };
  const rows = $derived.by((): { calories: string; main: Row[]; vitamins: Row[]; dv: boolean } | null => {
    if (own) {
      const v = own.values;
      const row = (k: string): Row => ({ key: k, label: (LABELS as Record<string, string>)[k] ?? human(k), amount: v[k], dv: null, sub: SUB.has(k) });
      const known = new Set<string>(['calories', ...NUTRIENTS]);
      const fats = ['polyunsaturated_fat', 'monounsaturated_fat', 'unsaturated_fat'];
      const main = [...MAIN.slice(0, 3), ...fats, ...MAIN.slice(3)].filter((k) => k in v).map(row);
      const vitamins = [...VITAMINS.filter((k) => k in v), ...Object.keys(v).filter((k) => !known.has(k) && !fats.includes(k))].map(row);
      return { calories: (v.calories ?? '').replace(/\s*(k?cal(ories)?)\s*$/i, ''), main, vitamins, dv: false };
    }
    if (!est?.counted) return null;
    const each = (k: Nutrient) => est.totals[k] / servings;
    const row = (k: Nutrient): Row => {
      const a = labelAmount(k, each(k));
      return { key: k, label: LABELS[k], amount: `${a}${/^less/.test(a) ? ' ' : ''}${UNIT[k]}`, dv: perServing && DAILY[k] ? dailyValue(k, each(k)) : null, sub: SUB.has(k) };
    };
    return { calories: labelAmount('calories', each('calories')), main: MAIN.map(row), vitamins: VITAMINS.map(row), dv: perServing };
  });

  const WHY: Record<string, string> = { 'no amount': 'no amount given', 'no unit': 'no unit given', 'not in the nutrition table': 'not in the nutrition list yet' };
</script>

{#if rows}
  <section class="nutrition" aria-labelledby="{id}-h">
    <div class="label">
      <h2 id="{id}-h">Nutrition Facts</h2>
      {#if est}<p class="estimate">Estimated from the ingredients</p>{/if}
      <div class="servings">
        {#if own}
          {#if own.serving}<p class="size"><span>Serving size</span> <span>{own.serving}</span></p>{/if}
        {:else}
          {#if yields}
            <p>{+servings.toFixed(1)} {plural(servings)} per recipe</p>
          {:else}
            <p class="split"><label for="{id}-n">Servings per recipe</label>
              <input id="{id}-n" class="field" type="number" inputmode="numeric" min="1" step="1" bind:value={split} aria-describedby="{id}-hint" /></p>
            <p class="hint muted" id="{id}-hint">This recipe doesn't say how many it serves. Choose how many servings to split it into.</p>
          {/if}
          <p class="size"><span>Serving size</span> <span>{servingSize}</span></p>
        {/if}
      </div>
      <div class="calories">
        <p class="per">{own || perServing ? 'Amount per serving' : 'Amount per recipe'}</p>
        <p class="cal"><span>Calories</span> <span>{rows.calories}</span></p>
      </div>
      <table>
        <caption class="visually-hidden">Nutrients {perServing || own ? 'per serving' : 'in the whole recipe'}</caption>
        {#if rows.dv}<thead><tr><td></td><th scope="col" class="dvh">% Daily Value*</th></tr></thead>{/if}
        <tbody>
          {#each rows.main as r (r.key)}
            <tr class:sub={r.sub}><th scope="row"><b class:plain={r.sub}>{#if r.key === 'trans_fat'}<i>Trans</i> Fat{:else}{r.label}{/if}</b> {r.amount}</th>{#if rows.dv}<td>{#if r.dv !== null}<b>{r.dv}%</b>{/if}</td>{/if}</tr>
          {/each}
        </tbody>
        {#if rows.vitamins.length}
          <tbody class="vitamins">
            {#each rows.vitamins as r (r.key)}
              <tr><th scope="row">{r.label} {r.amount}</th>{#if rows.dv}<td>{r.dv ?? ''}{#if r.dv !== null}%{/if}</td>{/if}</tr>
            {/each}
          </tbody>
        {/if}
      </table>
      {#if rows.dv}
        <p class="foot">* The % Daily Value (DV) tells you how much a nutrient in a serving of food contributes to a daily diet. 2,000 calories a day is used for general nutrition advice.</p>
      {/if}
    </div>
    {#if est}
      <div class="about small muted">
        <p>Worked out from {est.counted} of {est.lines} ingredients with values from USDA FoodData Central. Cooking, frying and draining aren't taken into account.</p>
        {#if est.skipped.length}
          <details>
            <summary>Not counted ({est.skipped.length})</summary>
            <ul>{#each est.skipped as s}<li>{s.text.replace(/\[\[([^\]|]+)(?:\|([^\]]*))?\]\]|\[([^\]]*)\]\([^)]*\)|[*_]/g, (_, a, b, c) => b ?? a ?? c ?? '')} <span>({WHY[s.why] ?? s.why})</span></li>{/each}</ul>
          </details>
        {/if}
      </div>
    {:else if own}
      <p class="about small muted">As given where the recipe came from.</p>
    {/if}
  </section>
{/if}

<style>
  /* The label's own type: the sans in heavy weights and the hairline-to-bar rules of the FDA format,
     drawn in the theme's ink on its surface so it reads in light and dark. */
  .nutrition { margin: 32px 0 24px; }
  .label {
    inline-size: min(100%, 20rem); padding: 4px 8px 6px; border: 1px solid var(--text); background: var(--surface);
    color: var(--text); font: 14px/1.3 var(--sans);
  }
  .label p { margin: 0; }
  h2 { font: 900 2.1rem/1 var(--sans); letter-spacing: -0.01em; margin: 0; }
  .estimate { font-size: 13px; }
  .servings { border-top: 1px solid var(--text); margin-top: 4px; padding-top: 2px; }
  .size { display: flex; justify-content: space-between; gap: 8px; font-weight: 800; font-size: 1rem; }
  .split { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 2px 0; }
  .split input { inline-size: 5rem; font-size: max(1rem, 1em); min-height: 36px; padding: 4px 8px; text-align: end; }
  .hint { font-size: 12px; margin: 2px 0 4px !important; }
  .calories { border-top: 10px solid var(--text); padding-top: 2px; }
  .per { font-weight: 800; font-size: 13px; }
  .cal { display: flex; justify-content: space-between; align-items: baseline; font-weight: 900; font-size: 1.9rem; line-height: 1.05; }
  .cal span:first-child { font-size: 1.55rem; }
  table { inline-size: 100%; border-collapse: collapse; border-top: 5px solid var(--text); margin-top: 2px; }
  th, td { padding: 2px 0; border-top: 1px solid var(--text); text-align: start; font-weight: 400; vertical-align: baseline; }
  td { text-align: end; padding-inline-start: 8px; }
  thead th, thead td { border-top: 0; }
  .dvh { text-align: end; font-weight: 800; font-size: 12px; }
  tr.sub th { padding-inline-start: 1rem; }
  b { font-weight: 800; }
  b.plain { font-weight: 400; }
  tbody tr:first-child th, tbody tr:first-child td { border-top: 0; }
  .vitamins tr:first-child th, .vitamins tr:first-child td { border-top: 10px solid var(--text); }
  .foot { border-top: 5px solid var(--text); margin-top: 2px !important; padding-top: 4px; font-size: 11px; line-height: 1.35; }
  .about { max-width: 34rem; margin-top: 8px; }
  .about p { margin: 0 0 4px; }
  details ul { margin: 4px 0 0; padding-inline-start: 18px; }
  summary { cursor: pointer; color: var(--accent-ink); inline-size: fit-content; }
  summary:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 4px; }
  .small { font-size: 13px; }
  .visually-hidden { position: absolute; clip-path: inset(50%); overflow: hidden; width: 1px; height: 1px; margin: -1px; padding: 0; border: 0; white-space: nowrap; }
</style>
