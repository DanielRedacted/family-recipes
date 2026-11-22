// File-manager actions shared by the tree, folder view and note toolbar.
import { isMap, parseDocument } from 'yaml';
import { api, saveFile } from './api';
import { setProp } from './frontmatter';
import { stamp, today } from './credit';
import { ask, choose, confirmIt } from './dialog.svelte';
import { app, dirOf, go, loadSession, loadVault, nameOf, noteHash, folderHash, toast, canWrite, newTab, retarget, wide, favorites, setFavorites, type FolderMeta } from './state.svelte';
import { openMenu, type MenuItem } from './menu.svelte';
import { importer, enqueueFiles } from './importer.svelte';
import { picker } from './picker.svelte';
import { DAYS, proteinsOf, type Made } from './made';
import { tagsOf } from './search';
import type { Entry } from './types';

/** "Made today": confirm, then log the meal (any device may; the recipe file isn't changed). */
export async function markMade(e: Entry) {
  const tags = tagsOf(e);
  const prot = tags.some((t) => t.toLowerCase() === 'dinner') ? proteinsOf(tags) : [];
  const note = prot.length ? `${prot.join(' and ')} dinners won’t be suggested for the next ${DAYS} days.` : undefined;
  if (!(await confirmIt(`Made “${e.title}” today?`, note, 'Made today'))) return;
  try {
    app.made = await api<Made[]>('POST', '/api/made', { path: e.path });
    toast(`Logged ${e.title}`, 'ok');
  } catch (err: any) { toast(err.message, 'error'); }
}

/** Log a guess, quietly: the recipe stayed open long enough to have been cooked. */
export async function assumeMade(path: string) {
  try { app.made = await api<Made[]>('POST', '/api/made', { path, assumed: true }); } catch { /* offline or gone: no guess */ }
}

/** "Yes, cooked" (keeps the guess's time) or "Not cooked" (removes it). */
export async function settleGuess(m: Made, cooked: boolean) {
  try {
    app.made = await api<Made[]>(cooked ? 'PUT' : 'DELETE', `/api/made/${m.at}?path=${encodeURIComponent(m.path)}`);
    toast(cooked ? `Logged ${m.title}` : `Removed the guess for ${m.title}`, 'ok');
  } catch (err: any) { toast(err.message, 'error'); }
}

const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);

export function requireWrite(): boolean {
  if (canWrite()) return true;
  toast('Editing from this device needs a family PIN. Set one in the Recipes app on the Mac, then sign in again.', 'error');
  return false;
}

// ---------- who is credited for a change (lib/credit.ts) ----------

let warnedUnsigned = false;
/**
 * The full name to credit for a change from here: the signed-in person, or on the Mac the person
 * chosen as "Editing as" (asked the first time). '' = nobody to credit; null = the person cancelled.
 */
export async function signer(): Promise<string | null> {
  const u = app.session.user;
  if (u && u.id !== 'admin') return u.full_name || u.name;
  if (app.session.role !== 'admin') return '';
  let people: { id: string; name: string; full_name: string }[] = [];
  try { people = await api('GET', '/api/users'); } catch {}
  if (!people.length) {
    if (!warnedUnsigned) toast('Changes made on the Mac are credited once someone is added in Settings → People.');
    warnedUnsigned = true;
    return '';
  }
  const label = (p: (typeof people)[number]) => (p.full_name && p.full_name !== p.name ? `${p.name} (${p.full_name})` : p.name);
  const pick = await choose('Who is editing on this Mac?', people.map(label), '', 'Continue');
  const p = people.find((x) => label(x) === pick);
  if (!p) return null;
  await setEditingAs(p.id);
  return p.full_name || p.name;
}

/** The person the Mac edits as ('' = nobody, the built-in Admin). */
export async function setEditingAs(id: string) {
  try { await api('PUT', '/api/settings', { editing_as: id }); await loadSession(); } catch (e: any) { toast(e.message, 'error'); }
}

const cleanName = (s: string) => s.trim().replace(/[\/\\:]/g, '-').replace(/^\.+/, '');

export const RECIPE_TEMPLATE = (title: string) => `# ${title}\n\n## Ingredients\n\n- \n\n## Directions\n\n1. \n\n## Notes\n\n`;

export async function newNote(dir: string) {
  if (!requireWrite()) return;
  const title = await ask('New recipe', '', 'Create', dir ? `In ${dir}` : undefined);
  if (!title?.trim()) return;
  const by = await signer();
  if (by === null) return;
  try {
    const text = stamp(RECIPE_TEMPLATE(title.trim()), { by, date: today(), note: 'Created', author: true });
    const r = await saveFile(join(dir, cleanName(title) + '.md'), text, null, true);
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
    if (isFolder) await renameFolderMeta(from, r.path);
    await loadVault();
    retarget(from, r.path);
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
    retarget(path, null);
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

/** Open a web page in the browser: from the desktop app via the system, elsewhere in a new tab. */
export function openExternal(url: string) {
  const tab = () => window.open(url, '_blank', 'noopener');
  if (app.session.shell) api('POST', '/api/shell/open-url', { url }).catch(tab);
  else tab();
}

export async function shell(action: string, body: any = {}) {
  try { return await api('POST', `/api/shell/${action}`, body); } catch (e: any) { toast(e.message, 'error'); }
}

// ---------- menus ----------

const IMPORTABLE = '.pdf,.docx,.doc,.rtf,.odt,.html,.htm,.txt,.md,image/*';

/** Open the system file chooser (must run inside a click). */
export function pickFiles(accept: string, cb: (files: FileList) => void) {
  const i = document.createElement('input');
  i.type = 'file';
  i.multiple = true;
  if (accept) i.accept = accept;
  i.onchange = () => { if (i.files?.length) cb(i.files); };
  i.click();
}

/** The single "+" button: new recipe (blank or imported), files, folder. */
export function plusMenu(e: MouseEvent, dir: string) {
  if (!requireWrite()) return;
  const toImport = (focus: 'url' | 'text') => () => { importer.folder = dir; importer.focus = focus; go('#/import'); };
  openMenu(e, [
    { label: 'Blank recipe', run: () => newNote(dir) },
    { label: 'From a web address…', run: toImport('url') },
    { label: 'From a file (PDF, Word, photo…)…', run: () => pickFiles(IMPORTABLE, (f) => { importer.folder = dir; importer.selected = null; enqueueFiles(f); go('#/import'); }) },
    { label: 'Paste recipe text…', run: toImport('text') },
    { label: 'Upload files here as they are…', run: () => pickFiles('', (f) => upload(dir, f)), sep: true },
    { label: 'New folder', run: () => newFolder(dir) },
  ]);
}

/** Actions for a file or folder (tree, cards, list rows). */
export function entryItems(path: string, isFolder: boolean): MenuItem[] {
  const w = canWrite();
  const it: MenuItem[] = [];
  if (wide()) it.push({ label: 'Open in new tab', run: () => newTab(isFolder ? folderHash(path) : noteHash(path)) });
  if (w && isFolder) it.push({ label: 'New recipe here', run: () => newNote(path) }, { label: 'New folder here', run: () => newFolder(path) });
  if (w && (isFolder || path.endsWith('.md'))) it.push({ label: 'Icon & colour…', run: () => openPicker(path, isFolder), sep: isFolder });
  if (path.endsWith('.md')) it.push({ label: isFavorite(path) ? 'Remove from favorites' : 'Add to favorites', run: () => toggleFavorite(path) });
  if (w && path.endsWith('.md')) it.push({ label: isBookmarked(path) ? 'Remove bookmark' : 'Bookmark for everyone', run: () => toggleBookmark(path) });
  if (w) it.push({ label: 'Rename…', run: () => rename(path, isFolder) }, { label: 'Move to…', run: () => moveDialog(path, isFolder) });
  if (app.session.shell) it.push({ label: 'Show in Finder', run: () => shell('reveal', { path }) });
  if (w) it.push({ label: 'Move to trash', run: () => remove(path, isFolder), danger: true, sep: true });
  if (it[0]?.label === 'Open in new tab' && it[1]) it[1] = { ...it[1], sep: true };
  return it;
}

// ---------- bookmarks are everyone's (`favorite: true` in the recipe's properties, the key kept from
// before); favorites are each person's own (kept in this browser, see state.svelte.ts) ----------

export const isFavorite = (path: string) => favorites.paths.includes(path);
export const toggleFavorite = (path: string) => setFavorites(isFavorite(path) ? favorites.paths.filter((p) => p !== path) : [...favorites.paths, path]);

export const isBookmarked = (path: string) => app.byPath[path]?.frontmatter?.favorite === true;

export async function toggleBookmark(path: string) {
  if (!requireWrite()) return;
  const e = app.byPath[path];
  if (!e || e.kind !== 'note') return;
  try {
    await saveFile(path, setProp(e.raw ?? '', 'favorite', isBookmarked(path) ? undefined : true), e.etag);
    await loadVault();
  } catch (err: any) { toast(err.message, 'error'); }
}

// ---------- icons and colours ----------

export function openPicker(path: string, folder: boolean) {
  if (!requireWrite()) return;
  picker.path = path;
  picker.folder = folder;
}

const META_HEADER = '# Folder icons and card colours (edited by the Recipes app; safe to edit by hand)\n';

async function writeFolderMeta(edit: (doc: ReturnType<typeof parseDocument>) => void) {
  const doc = parseDocument(app.folderMetaRaw.trim() ? app.folderMetaRaw : META_HEADER);
  // a file with only the header comment (or "null") has no map yet to add folders to
  if (!isMap(doc.contents)) doc.contents = doc.createNode({}) as unknown as typeof doc.contents;
  edit(doc);
  await api('PUT', '/api/folders', String(doc));
}

export async function setFolderMeta(path: string, meta: FolderMeta) {
  try {
    await writeFolderMeta((doc) => {
      for (const k of ['emoji', 'color'] as const) {
        if (meta[k]) doc.setIn([path, k], meta[k]);
        else if (doc.hasIn([path, k])) doc.deleteIn([path, k]);
      }
      if (doc.has(path) && (doc.get(path) as any)?.items?.length === 0) doc.delete(path);
    });
    await loadVault();
  } catch (e: any) { toast(e.message, 'error'); }
}

/** Keep a folder's icon/colour (and its subfolders') when it is renamed or moved. */
async function renameFolderMeta(from: string, to: string) {
  const keys = Object.keys(app.folderMeta).filter((k) => k === from || k.startsWith(from + '/'));
  if (!keys.length || from === to) return;
  await writeFolderMeta((doc) => {
    for (const k of keys) {
      const v = doc.get(k);
      doc.delete(k);
      doc.set(to + k.slice(from.length), v);
    }
  }).catch(() => {});
}
