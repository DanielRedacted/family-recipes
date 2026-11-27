// App updates (desktop app only): the app checks GitHub Releases when it opens, every 6 hours in the
// background (lib.rs `spawn_update_checks`, which sends an `update` event when one is ready), and on
// request from Settings → App. Settings → App → Updates says whether a new version is only announced,
// downloaded in the background, or installed in the background. Installing restarts the app.
import { api } from './api';
import { app, mayLeave, toast } from './state.svelte';

export type UpdateMode = 'notify' | 'download' | 'install';

export const update = $state({ version: '', notes: '', ready: false, mode: 'notify' as UpdateMode, busy: false, checking: false, hidden: false });

export async function checkForUpdate(quiet = false) {
  update.checking = true;
  try {
    const r = await api<{ version: string | null; notes: string | null; ready: boolean; mode: UpdateMode }>('POST', '/api/shell/update-check');
    update.version = r.version ?? '';
    update.notes = r.notes ?? '';
    update.ready = r.ready;
    update.mode = r.mode;
    update.hidden = false;
    if (!quiet && !r.version) toast(`Recipes ${app.session.version} is the latest version`, 'ok');
  } catch (e: any) {
    if (!quiet) toast(e.message, 'error');
  } finally {
    update.checking = false;
  }
  autoInstall();
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

/** "Install automatically": restart into an installed update once the window is closed and nothing is
 *  unsaved. The shell checks the window really is closed (a covered window also reads as hidden). */
export async function autoInstall() {
  if (!update.ready || update.mode !== 'install' || update.busy || !document.hidden || !(await mayLeave(true))) return;
  try { await api('POST', '/api/shell/update-install', { quiet: true }); } catch {}
}
