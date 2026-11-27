<script lang="ts">
  // The Calendar page (#/calendar): a week or a month of the family's history (meals cooked, guessed
  // meals, recipes added and changed) and, when switched on, the Mac's calendar events (read-only).
  import { getContext } from 'svelte';
  import Icon from './Icon.svelte';
  import Back from './Back.svelte';
  import { app, noteHash, type Tab } from '../lib/state.svelte';
  import { settleGuess } from '../lib/ops';
  import { ASSUME_AFTER_MIN } from '../lib/made';
  import { daysOf, keyOf, addDays, startOfDay, fetchEvents, eventsOn, history, timeOf, refreshAtNaturalTimes, type CalEvent, type Item } from '../lib/calendar';

  const tab = getContext<Tab>('tab');
  let view = $state<'week' | 'month'>((() => { try { return localStorage.getItem('calendarView') === 'month' ? 'month' : 'week'; } catch { return 'week'; } })());
  $effect(() => { try { localStorage.setItem('calendarView', view); } catch {} });

  let anchor = $state(startOfDay(new Date()));
  let today = $state(keyOf(new Date()));
  let picked = $state(keyOf(new Date()));
  let events = $state<CalEvent[]>([]);

  const days = $derived(daysOf(view, anchor));
  const past = $derived(history(app.made, app.files));
  const shown = $derived(view === 'month' ? days.filter((d) => d.getMonth() === anchor.getMonth()) : days);

  /** everything on a day: the Mac's events, then the app's history */
  function itemsOn(d: Date): Item[] {
    return [...eventsOn(events, d).map((ev): Item => ({ kind: 'event', ev })), ...(past.get(keyOf(d)) ?? [])];
  }

  const fmt = (d: Date, o: Intl.DateTimeFormatOptions) => d.toLocaleDateString(undefined, o);
  const heading = $derived.by(() => {
    if (view === 'month') return fmt(anchor, { month: 'long', year: 'numeric' });
    // "Nov 22 – 28, 2026", "Nov 29 – Dec 5, 2026", as the language writes a range
    return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', year: 'numeric' }).formatRange(days[0], days[6]);
  });

  function move(n: number) {
    anchor = view === 'week' ? addDays(anchor, 7 * n) : new Date(anchor.getFullYear(), anchor.getMonth() + n, 1);
    const first = view === 'week' ? days[0] : anchor;
    picked = shown.some((d) => keyOf(d) === today) ? today : keyOf(first);
  }
  function goToday() { anchor = startOfDay(new Date()); picked = today = keyOf(new Date()); }

  // the Mac's events for the days on screen; asked again every 15 minutes and when the page comes back
  async function load() {
    if (!app.session.calendar || !app.session.user) { events = []; return; }
    if (tab.id !== app.tab || document.visibilityState !== 'visible') return;
    today = keyOf(new Date());
    events = await fetchEvents(days[0], addDays(days[days.length - 1], 1));
  }
  $effect(() => {
    days; app.session.calendar; app.tab;
    return refreshAtNaturalTimes(load);
  });

  const clock = (s: number) => new Date(s * 1000).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  const exists = (p: string) => !!app.byPath[p];
  const label = (it: Item) =>
    it.kind === 'event' ? it.ev.title
    : it.kind === 'cooked' ? (it.made.assumed ? `${it.made.title} (assumed)` : it.made.title)
    : it.title;
  const icon = (it: Item) => (it.kind === 'event' ? 'clock' : it.kind === 'cooked' ? 'utensils' : it.kind === 'added' ? 'plus' : 'pencil');
  const dayName = (d: Date) => fmt(d, { weekday: 'long', month: 'long', day: 'numeric' });
  const MAX = 3; // items shown in a month cell
  const weekdays = $derived(days.slice(0, 7).map((d) => fmt(d, { weekday: 'short' })));
</script>

{#snippet link(path: string, title: string)}
  {#if exists(path)}<a href={noteHash(path)}>{title}</a>{:else}<span>{title}</span>{/if}
{/snippet}

{#snippet entry(it: Item, d: Date)}
  {#if it.kind === 'event'}
    <li class="ev">
      <span class="t small">{timeOf(it.ev, d)}</span>
      <span class="name">{it.ev.title}</span>
      {#if it.ev.location}<span class="muted small">{it.ev.location}</span>{/if}
    </li>
  {:else if it.kind === 'cooked'}
    <li class="log" class:guess={it.made.assumed}>
      <Icon name="utensils" size={16} />
      <span class="txt">
        {#if it.made.assumed}
          Probably cooked {@render link(it.made.path, it.made.title)} <span class="assumed small">Assumed</span>
          <span class="muted small sub">{[it.made.by, clock(it.made.at), `open ${ASSUME_AFTER_MIN}+ min`].filter(Boolean).join(' · ')}</span>
          <span class="row acts">
            <button class="btn small" onclick={() => settleGuess(it.made, true)}>Yes, cooked</button>
            <button class="btn ghost small" onclick={() => settleGuess(it.made, false)}>Not cooked</button>
          </span>
        {:else}
          Cooked {@render link(it.made.path, it.made.title)}
          <span class="muted small sub">{[it.made.by, clock(it.made.at)].filter(Boolean).join(' · ')}</span>
        {/if}
      </span>
    </li>
  {:else if it.kind === 'added'}
    <li class="log">
      <Icon name="plus" size={16} />
      <span class="txt">{it.by || 'Someone'} added {@render link(it.path, it.title)}</span>
    </li>
  {:else}
    <li class="log">
      <Icon name="pencil" size={16} />
      <span class="txt">{it.by || 'Someone'} changed {@render link(it.path, it.title)}{#if it.note}<span class="muted small sub">{it.note}</span>{/if}</span>
    </li>
  {/if}
{/snippet}

{#snippet dayList(d: Date, items: Item[])}
  {#if items.length}
    <ul class="items">{#each items as it, i (i)}{@render entry(it, d)}{/each}</ul>
  {:else}
    <p class="muted small none">Nothing logged</p>
  {/if}
{/snippet}

<div class="page">
  <h1 class="title"><Back />Calendar</h1>
  <div class="bar row">
    <h2 class="period" aria-live="polite">{heading}</h2>
    <span class="spacer"></span>
    <div class="row nav">
      <button class="btn icon" aria-label="Previous {view}" onclick={() => move(-1)}><Icon name="chevron-left" size={16} /></button>
      <button class="btn" onclick={goToday}>Today</button>
      <button class="btn icon" aria-label="Next {view}" onclick={() => move(1)}><Icon name="chevron-right" size={16} /></button>
    </div>
    <div class="seg" role="group" aria-label="Show">
      <button class="btn" class:on={view === 'week'} aria-pressed={view === 'week'} onclick={() => (view = 'week')}>Week</button>
      <button class="btn" class:on={view === 'month'} aria-pressed={view === 'month'} onclick={() => (view = 'month')}>Month</button>
    </div>
  </div>
  {#if app.session.role === 'admin' && !app.session.calendar}
    <p class="muted small hint">To show events from this Mac’s calendar here too, turn them on in <a href="#/settings/calendar">Settings → Calendar</a>.</p>
  {/if}

  {#if view === 'week'}
    <ol class="week">
      {#each days as d (keyOf(d))}
        {@const k = keyOf(d)}
        <li class="col" class:today={k === today}>
          <h3 class="dh"><span class="wd">{fmt(d, { weekday: 'short' })}</span> <span class="dn">{d.getDate()}</span><span class="sr"> {dayName(d)}</span></h3>
          {@render dayList(d, itemsOn(d))}
        </li>
      {/each}
    </ol>
  {:else}
    <div class="month">
      <div class="wk head" aria-hidden="true">{#each weekdays as w}<span class="muted small">{w}</span>{/each}</div>
      {#each Array.from({ length: days.length / 7 }, (_, i) => days.slice(i * 7, i * 7 + 7)) as week, wi (wi)}
        <div class="wk">
          {#each week as d (keyOf(d))}
            {@const k = keyOf(d)}
            {@const items = itemsOn(d)}
            <button class="cell" class:out={d.getMonth() !== anchor.getMonth()} class:today={k === today} class:picked={k === picked}
              aria-pressed={k === picked} aria-label="{dayName(d)}: {items.length ? `${items.length} ${items.length === 1 ? 'entry' : 'entries'}` : 'nothing logged'}"
              onclick={() => (picked = k)}>
              <span class="dn">{d.getDate()}</span>
              {#if items.length}
                <span class="peek">
                  {#each items.slice(0, MAX) as it, i (i)}
                    <span class="pk" class:ev={it.kind === 'event'} class:guess={it.kind === 'cooked' && it.made.assumed}><Icon name={icon(it)} size={12} /><span>{label(it)}</span></span>
                  {/each}
                  {#if items.length > MAX}<span class="more small">+{items.length - MAX} more</span>{/if}
                </span>
                <span class="count small" aria-hidden="true">{items.length}</span>
              {/if}
            </button>
          {/each}
        </div>
      {/each}
    </div>
    {@const pd = shown.find((d) => keyOf(d) === picked) ?? shown[0]}
    <section class="picked-day" aria-label={dayName(pd)}>
      <h3>{dayName(pd)}</h3>
      {@render dayList(pd, itemsOn(pd))}
    </section>
  {/if}
</div>

<style>
  .page { padding: 16px 28px 80px; max-width: 1200px; }
  .title { display: flex; align-items: center; gap: 4px; margin: 8px 0 4px -8px; }
  .bar { margin: 8px 0 16px; gap: 12px; }
  .period { margin: 0; font: 500 1.35rem var(--serif); }
  .nav { gap: 4px; flex-wrap: nowrap; }
  .hint { margin: -4px 0 16px; }
  .seg { display: flex; }
  .seg .btn { border-radius: 0; }
  .seg .btn:first-child { border-radius: 8px 0 0 8px; }
  .seg .btn:last-child { border-radius: 0 8px 8px 0; border-left: 0; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }

  /* items, in a week column and in the chosen day of a month */
  .items { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .ev { background: var(--p-blue); border-radius: var(--radius); padding: 8px 10px; display: grid; gap: 2px; }
  .ev .t { color: var(--muted); font-variant-numeric: tabular-nums; }
  .ev .name { font-family: var(--serif); font-size: 16px; line-height: 1.3; overflow-wrap: anywhere; }
  .log { display: grid; grid-template-columns: 16px 1fr; gap: 8px; align-items: start; padding: 2px 0; font-size: 14px; line-height: 1.4; }
  .log :global(svg) { color: var(--muted); margin-top: 2px; }
  .log a { color: var(--accent-ink); text-decoration: none; }
  .log a:hover { text-decoration: underline; }
  .txt { min-width: 0; overflow-wrap: anywhere; }
  .sub { display: block; margin-top: 2px; }
  .guess { color: var(--muted); }
  .assumed { border: 1px dashed var(--muted); border-radius: 16px; padding: 0 8px; margin-left: 4px; white-space: nowrap; }
  .acts { margin-top: 6px; gap: 6px; }
  .acts .btn { min-height: 32px; padding: 4px 10px; }
  .none { margin: 0; }

  /* week: seven columns */
  .week { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 12px; }
  .col { min-width: 0; display: grid; align-content: start; gap: 8px; }
  .dh { margin: 0; padding-bottom: 6px; border-bottom: 1px solid var(--border); font: 500 13px var(--sans); color: var(--muted); display: flex; align-items: baseline; gap: 6px; }
  .dh .dn { font: 500 1.25rem var(--serif); color: var(--text); }
  .col.today .dh { border-bottom-color: var(--accent); }
  .col.today .dh .dn, .col.today .dh .wd { color: var(--accent-ink); font-weight: 600; }

  /* month: a grid of day buttons, the chosen day's list below */
  .month { display: grid; gap: 4px; }
  .wk { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 4px; }
  .wk.head span { padding: 0 6px; }
  .cell { font: inherit; color: inherit; text-align: left; min-height: 104px; min-width: 0; padding: 6px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); cursor: pointer; display: flex; flex-direction: column; gap: 4px; }
  .cell:hover { border-color: var(--accent); }
  .cell.out { background: var(--sunken); color: var(--muted); }
  .cell.picked { background: var(--accent-soft); border-color: var(--accent); }
  .cell .dn { font-size: 14px; font-variant-numeric: tabular-nums; }
  .cell.today .dn { color: var(--accent-ink); font-weight: 700; }
  .peek { display: grid; gap: 2px; min-width: 0; }
  .pk { display: flex; gap: 4px; align-items: center; font-size: 12px; line-height: 1.3; min-width: 0; }
  .pk span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pk :global(svg) { flex: none; color: var(--muted); }
  .pk.ev { background: var(--p-blue); border-radius: 4px; padding: 0 4px; }
  .pk.guess { color: var(--muted); font-style: italic; }
  .more { color: var(--muted); font-size: 12px; }
  .count { display: none; }
  .picked-day { margin-top: 24px; max-width: 640px; }
  .picked-day h3 { margin: 0 0 12px; font: 500 1.1rem var(--serif); }

  @media (max-width: 899px) {
    .page { padding: 12px 16px 80px; }
    .bar { gap: 8px; }
    .period { flex-basis: 100%; order: -1; font-size: 1.2rem; }
    .bar .spacer { display: none; }
    /* a week becomes a list of days */
    .week { grid-template-columns: 1fr; gap: 20px; }
    /* a month's cells shrink to the date and a count */
    .cell { min-height: 52px; padding: 4px; align-items: center; }
    .peek { display: none; }
    .count { display: block; color: var(--muted); font-variant-numeric: tabular-nums; }
    .cell.picked .count { color: var(--accent-ink); }
  }
  @media (hover: none) { .cell:hover { border-color: var(--border); } .cell.picked:hover { border-color: var(--accent); } }
</style>
