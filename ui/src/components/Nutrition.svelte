<script lang="ts">
  // `nutrition:` frontmatter map (written by the importer; see docs/FORMAT.md), shown as a small table.
  let { data }: { data: unknown } = $props();

  const rows = $derived.by(() => {
    if (!data || typeof data !== 'object' || Array.isArray(data)) return [];
    return Object.entries(data as Record<string, unknown>)
      .filter(([k, v]) => k !== 'serving_size' && v !== null && v !== '' && typeof v !== 'object')
      .map(([k, v]) => [k.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase()), String(v)]);
  });
  const serving = $derived(data && typeof data === 'object' ? (data as any).serving_size : null);
</script>

{#if rows.length}
  <section class="nutrition">
    <h2>Nutrition{#if serving} <span class="muted small">per {serving}</span>{/if}</h2>
    <table>
      <tbody>{#each rows as [k, v]}<tr><th>{k}</th><td>{v}</td></tr>{/each}</tbody>
    </table>
  </section>
{/if}

<style>
  .nutrition { max-width: 420px; margin: 18px 0; }
  h2 { font-size: 1.15rem; margin: 0 0 6px; }
  table { border-collapse: collapse; width: 100%; font-size: 14px; }
  th { text-align: left; font-weight: 500; color: var(--muted); padding: 3px 12px 3px 0; }
  td { text-align: right; padding: 3px 0; font-variant-numeric: tabular-nums; }
  tr + tr { border-top: 1px solid var(--border); }
</style>
