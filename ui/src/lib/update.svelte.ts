// App updates (desktop app only): the app checks GitHub Releases once when it opens, and on
// request from Settings → App. Installing restarts the app into the new version.
import { api } from './api';
import { app, mayLeave, toast } from './state.svelte';

export const update = $state({ version: '', notes: '', busy: false, hidden: false });

export async function checkForUpdate(quiet = false) {
  try {
    const r = await api<{ version: string | null; notes: string | null }>('POST', '/api/shell/update-check');
    update.version = r.version ?? '';
    update.notes = r.notes ?? '';
    update.hidden = false;
    if (!quiet && !r.version) toast(`Recipes ${app.session.version} is the latest version`, 'ok');
  } catch (e: any) {
    if (!quiet) toast(e.message, 'error');
  }
}

export async function installUpdate() {
  if (!(await mayLeave())) return;
  update.busy = true;
  try {
    await api('POST', '/api/shell/update-install');
    toast('Restarting into the new version…');
  } catch (e: any) {
    update.busy = false;
    toast(e.message, 'error');
  }
}
