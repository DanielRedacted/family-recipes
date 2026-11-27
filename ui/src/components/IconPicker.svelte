<script lang="ts">
  import { untrack } from 'svelte';
  import { picker } from '../lib/picker.svelte';
  import { app, loadVault, nameOf, stemOf, dirOf } from '../lib/state.svelte';
  import { suggestEmoji, iconOf, COLORS } from '../lib/icons';
  import { setFolderMeta } from '../lib/ops';
  import { saveFile } from '../lib/api';
  import { setProp } from '../lib/frontmatter';
  import CardIcon from './CardIcon.svelte';
  import { modal } from '../lib/modal';

  let emoji = $state(''); // '' = automatic, 'none' = no icon
  let color = $state(''); // '' = automatic
  let busy = $state(false);
  let error = $state(''); // shown in the form: a toast would sit under the dialog

  const name = $derived(picker.path ? (picker.folder ? nameOf(picker.path) : app.byPath[picker.path]?.title ?? stemOf(picker.path)) : '');
  const folderIcon = $derived(picker.path && !picker.folder ? iconOf(nameOf(dirOf(picker.path)), app.folderMeta[dirOf(picker.path)]?.emoji) : null);
  const auto = $derived(iconOf(name, undefined, folderIcon));
  const suggestions = $derived(suggestEmoji(name, 30));

  $effect(() => {
    const p = picker.path;
    if (!p) return;
    untrack(() => {
      const cur = picker.folder ? app.folderMeta[p] : app.byPath[p]?.frontmatter;
      emoji = typeof cur?.emoji === 'string' ? cur.emoji : '';
      color = typeof cur?.color === 'string' ? cur.color : '';
    });
  });

  const close = () => { picker.path = null; error = ''; };

  async function save(e: Event) {
    e.preventDefault();
    const p = picker.path;
    if (!p) return;
    busy = true;
    error = '';
    try {
      if (picker.folder) await setFolderMeta(p, { emoji: emoji.trim() || undefined, color: color || undefined });
      else {
        const entry = app.byPath[p];
        if (!entry) throw new Error('This recipe is no longer there');
        const raw = setProp(setProp(entry.raw ?? '', 'emoji', emoji.trim() || undefined), 'color', color || undefined);
        if (raw !== entry.raw) { await saveFile(p, raw, entry.etag); await loadVault(); }
      }
      close();
    } catch (err: any) { error = err.message; }
    finally { busy = false; }
  }
</script>

{#if picker.path}
  <dialog class="modal-host" aria-labelledby="picker-title" use:modal={{ onclose: close, outside: true }}>
      <form class="modal card" onsubmit={save}>
        <h3 id="picker-title">Icon & colour — {name}</h3>
        <div class="preview" style="background:{color ? `var(--p-${color})` : 'var(--sunken)'}">
          <CardIcon icon={emoji === 'none' ? null : emoji.trim() ? { emoji: emoji.trim() } : auto} size={40} />
        </div>
        <fieldset>
          <legend>Icon</legend>
          <div class="row">
            <button type="button" class="btn small" class:on={emoji === ''} onclick={() => (emoji = '')}>Automatic{#if auto} (<CardIcon icon={auto} size={14} />){/if}</button>
            <button type="button" class="btn small" class:on={emoji === 'none'} onclick={() => (emoji = 'none')}>None</button>
            <input class="field own" maxlength="16" placeholder="Type or paste any emoji" value={emoji === 'none' ? '' : emoji} oninput={(e) => (emoji = e.currentTarget.value)} aria-label="Emoji" />
          </div>
          <div class="grid">
            {#each suggestions as s}<button type="button" class="emo" class:on={emoji === s} onclick={() => (emoji = s)} aria-label="Use {s}">{s}</button>{/each}
          </div>
          <p class="muted small">On a Mac, Control-Command-Space opens every emoji.</p>
        </fieldset>
        <fieldset>
          <legend>Card colour</legend>
          <div class="row">
            <button type="button" class="btn small" class:on={color === ''} onclick={() => (color = '')}>Automatic</button>
            {#each COLORS as c}<button type="button" class="sw" class:on={color === c} style="background:var(--p-{c})" onclick={() => (color = c)} aria-label={c} title={c}></button>{/each}
          </div>
        </fieldset>
        {#if error}<p class="form-error" role="alert">{error}</p>{/if}
        <div class="row end">
          <button type="button" class="btn" onclick={close}>Cancel</button>
          <button class="btn primary" disabled={busy}>Save</button>
        </div>
      </form>
  </dialog>
{/if}

<style>
  .modal { width: min(480px, calc(100vw - 32px)); padding: 20px; display: grid; gap: 12px; max-height: 90vh; overflow: auto; }
  h3 { margin: 0; }
  .preview { height: 72px; border-radius: var(--radius); display: grid; place-items: center; }
  fieldset { border: 0; padding: 0; margin: 0; display: grid; gap: 8px; }
  legend { font-weight: 600; margin-bottom: 8px; font-size: 14px; }
  .own { flex: 1; min-width: 140px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(38px, 1fr)); gap: 4px; }
  .emo { font-size: 22px; background: none; border: 1px solid transparent; border-radius: 8px; padding: 4px; cursor: pointer; }
  .emo:hover { background: var(--sunken); }
  .emo.on { border-color: var(--accent); background: var(--accent-soft); }
  .end { justify-content: flex-end; }
  p { margin: 0; }
</style>
