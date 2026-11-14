// File-manager actions shared by the tree, folder view and note toolbar.
import { api, saveFile } from './api';
import { ask, choose, confirmIt } from './dialog.svelte';
import { app, dirOf, go, loadVault, nameOf, noteHash, folderHash, toast, canWrite } from './state.svelte';

const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);

export function requireWrite(): boolean {
  if (canWrite()) return true;
  if (app.session.pin_set) app.pinPrompt = true;
  else toast('Editing is only available in the Recipes app on the Mac until a PIN is set there.', 'error');
  return false;
}

const cleanName = (s: string) => s.trim().replace(/[\/\\:]/g, '-').replace(/^\.+/, '');

export const RECIPE_TEMPLATE = (title: string) => `# ${title}\n\n## Ingredients\n\n- \n\n## Directions\n\n1. \n\n## Notes\n\n`;

export async function newNote(dir: string) {
  if (!requireWrite()) return;
  const title = await ask('New recipe', '', 'Create', dir ? `In ${dir}` : undefined);
  if (!title?.trim()) return;
  try {
    const r = await saveFile(join(dir, cleanName(title) + '.md'), RECIPE_TEMPLATE(title.trim()), null, true);
    await loadVault();
    go(noteHash(r.path, 'edit'));
  } catch (e: any) { toast(e.message, 'error'); }
}

export async function newFolder(parent: string) {
  if (!requireWrite()) return;
  const name = await ask('New folder', '', 'Create', parent ? `In ${parent}` : undefined);
  if (!name?.trim()) return;
  try {
    const r = await api('POST', '/api/fs/mkdir', { path: join(parent, cleanName(name)) });
    await loadVault();
    go(folderHash(r.path));
  } catch (e: any) { toast(e.message, 'error'); }
}

export async function rename(path: string, isFolder = false) {
  if (!requireWrite()) return;
  const old = nameOf(path);
  const ext = !isFolder && old.includes('.') ? old.slice(old.lastIndexOf('.')) : '';
  const stem = ext ? old.slice(0, -ext.length) : old;
  const name = await ask(isFolder ? 'Rename folder' : 'Rename', stem, 'Rename');
  if (!name?.trim() || name.trim() === stem) return;
  await moveTo(path, join(dirOf(path), cleanName(name) + ext), isFolder);
}

export async function moveTo(from: string, to: string, isFolder = false) {
  if (from === to) return;
  try {
    const r = await api('POST', '/api/fs/move', { from, to });
    await loadVault();
    const cur = app.route.path;
    if (cur === from || cur.startsWith(from + '/')) {
      const np = r.path + cur.slice(from.length);
      go(isFolder || app.route.view === 'folder' ? folderHash(np) : noteHash(np));
    }
    toast(`Moved to ${dirOf(r.path) || 'top level'}`, 'ok');
  } catch (e: any) { toast(e.message, 'error'); }
}

export async function moveDialog(path: string, isFolder = false) {
  if (!requireWrite()) return;
  const folders = ['', ...app.folders.filter((f) => f !== path && !f.startsWith(path + '/'))];
  const dest = await choose(`Move “${nameOf(path)}” to…`, folders, dirOf(path), 'Move');
  if (dest === null || dest === dirOf(path)) return;
  await moveTo(path, join(dest, nameOf(path)), isFolder);
}

export async function remove(path: string, isFolder = false) {
  if (!requireWrite()) return;
  const ok = await confirmIt(`Move “${nameOf(path)}” to the trash?`, isFolder ? 'Everything inside the folder goes with it. You can restore it from Trash.' : 'You can restore it from Trash.', 'Move to trash', true);
  if (!ok) return;
  try {
    await api('DELETE', '/api/files/' + path.split('/').map(encodeURIComponent).join('/'));
    await loadVault();
    if (app.route.path === path || app.route.path.startsWith(path + '/')) go(folderHash(dirOf(path)));
    toast('Moved to trash', 'ok');
  } catch (e: any) { toast(e.message, 'error'); }
}

export async function upload(dir: string, files: FileList | File[]) {
  if (!requireWrite()) return;
  let n = 0;
  for (const f of Array.from(files)) {
    try {
      await saveFile(join(dir, cleanName(f.name)), f, null, true);
      n++;
    } catch (e: any) { toast(`${f.name}: ${e.message}`, 'error'); }
  }
  if (n) { await loadVault(); toast(`Added ${n} file${n > 1 ? 's' : ''}`, 'ok'); }
}

/** Upload an image into attachments/ and return the markdown to insert. */
export async function attachImage(file: File, baseName: string): Promise<string | null> {
  const ext = (file.name.match(/\.[^.]+$/)?.[0] || '.jpg').toLowerCase();
  const name = cleanName(baseName).toLowerCase().replace(/\s+/g, '-') + ext;
  try {
    const r = await saveFile(`attachments/${name}`, file, null, true);
    await loadVault();
    return `![${baseName}](/${encodeURI(r.path)})`;
  } catch (e: any) { toast(e.message, 'error'); return null; }
}

export async function shell(action: string, body: any = {}) {
  try { return await api('POST', `/api/shell/${action}`, body); } catch (e: any) { toast(e.message, 'error'); }
}
