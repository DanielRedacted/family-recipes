<script lang="ts">
  // Today's events from the Mac's calendar, on the home page: only when the calendar is switched on
  // (Settings → Calendar) and today has events. Changed in Calendar.app, never here; asked again when
  // the page comes back to the front and every 15 minutes, so it follows edits and rolls over at midnight.
  import { getContext } from 'svelte';
  import { app, type Tab } from '../lib/state.svelte';
  import Section from './Section.svelte';
  import { renderInline } from '../lib/markdown';
  import { fetchEvents, eventsOn, timeOf, addDays, startOfDay, refreshAtNaturalTimes, type CalEvent } from '../lib/calendar';

  const tab = getContext<Tab>('tab');
  let events = $state<CalEvent[]>([]);
  let day = $state(startOfDay(new Date()));

  async function load() {
    if (!app.session.calendar || !app.session.user) { events = []; return; }
    if (tab.id !== app.tab || document.visibilityState !== 'visible') return;
    const d = startOfDay(new Date());
    const list = await fetchEvents(d, addDays(d, 1));
    day = d;
    events = eventsOn(list, d);
  }

  $effect(() => {
    app.session.calendar; app.tab; // reload when switched on, or when this tab comes back to the front
    return refreshAtNaturalTimes(load);
  });
</script>

{#if events.length}
  <Section id="today" class="today" title="Today" count={events.length}>
    {#snippet aside()}<a class="small cal" href="#/calendar">Calendar</a>{/snippet}
    <ul aria-label="Today's events">
      {#each events as e (e.title + e.start)}
        <li>
          <span class="when small">{timeOf(e, day)}</span>
          <span class="what">{@html renderInline(e.title)}{#if e.location}<span class="muted small where">{e.location}</span>{/if}</span>
        </li>
      {/each}
    </ul>
  </Section>
{/if}

<style>
  .cal { color: var(--accent-ink); text-decoration: none; padding: 8px 0; }
  .cal:hover { text-decoration: underline; }
  ul { list-style: none; margin: 0; padding: 0; background: var(--p-blue); border-radius: var(--radius); }
  li { display: grid; grid-template-columns: 9.5em 1fr; gap: 12px; padding: 10px 16px; align-items: baseline; }
  li + li { border-top: 1px solid var(--border); }
  .when { color: var(--muted); font-variant-numeric: tabular-nums; }
  .what { font-family: var(--serif); font-size: 17px; line-height: 1.35; overflow-wrap: anywhere; }
  .where { display: block; font-family: var(--sans); margin-top: 2px; }
  @media (max-width: 899px) {
    li { grid-template-columns: 1fr; gap: 2px; }
  }
</style>
