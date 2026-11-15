// Usage: node scripts/release.mjs 0.2.0 "What changed, one line per item"
// Builds Recipes.app on this Mac (Apple silicon + Intel), signs the update with the key in ~/.tauri,
// commits the version, tags vX.Y.Z (the tag message becomes the release notes), pushes, and publishes
// a GitHub release with the disk image, the update bundle and latest.json. Installed apps then offer it.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { homedir } from 'node:os';

const [v, notes] = process.argv.slice(2);
const die = (msg) => { console.error(msg); process.exit(1); };
if (!/^\d+\.\d+\.\d+$/.test(v || '')) die('usage: node scripts/release.mjs X.Y.Z "release notes"');
if (!notes) die('Add release notes as the second argument; they show up in the app under "What\'s new".');

const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: 'utf8', ...opts });
const git = (...a) => run('git', a).trim();
const key = `${homedir()}/.tauri/family-recipes.key`;
if (!existsSync(key)) die(`No signing key at ${key}. Restore it from your backup; without it installed apps reject updates.`);
if (git('status', '--porcelain')) die('Commit or stash your changes first.');
if (git('tag', '-l', `v${v}`)) die(`v${v} already exists.`);

const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const repo = conf.plugins.updater.endpoints[0].match(/github\.com\/([^/]+\/[^/]+)\/releases/)[1];
const cargo = readFileSync('Cargo.toml', 'utf8');
const current = cargo.match(/^version = "([^"]+)"/m)[1];
const newer = (a, b) => { const x = a.split('.').map(Number), y = b.split('.').map(Number); for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i]; return false; };
if (!newer(v, current)) die(`${v} must be newer than the current ${current}, or installed apps won't see it as an update.`);

// the version lives in Cargo.toml (tauri.conf.json reads it from there) and Cargo.lock
const edit = (file, re, to) => {
  const s = readFileSync(file, 'utf8'), t = s.replace(re, to);
  if (s === t) die(`Couldn't find the version in ${file}`);
  writeFileSync(file, t);
};
edit('Cargo.toml', /^version = "[^"]+"/m, `version = "${v}"`);
edit('Cargo.lock', /(name = "recipes-(?:app|core)"\nversion = )"[^"]+"/g, `$1"${v}"`);

const target = 'universal-apple-darwin';
try {
  execFileSync('ui/node_modules/.bin/tauri', ['build', '--target', target], {
    stdio: 'inherit',
    env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY: readFileSync(key, 'utf8'), TAURI_SIGNING_PRIVATE_KEY_PASSWORD: readFileSync(`${key}.password`, 'utf8') },
  });
} catch {
  git('checkout', '--', 'Cargo.toml', 'Cargo.lock');
  die('Build failed; the version was put back. Nothing was committed or published.');
}

const out = `target/${target}/release/bundle`;
const bundle = `${out}/macos/Recipes.app.tar.gz`;
const dmg = `${out}/dmg/Recipes_${v}_universal.dmg`;
for (const f of [bundle, `${bundle}.sig`, dmg]) if (!existsSync(f)) die(`Missing ${f}`);
const mac = { url: `https://github.com/${repo}/releases/download/v${v}/Recipes.app.tar.gz`, signature: readFileSync(`${bundle}.sig`, 'utf8') };
const latest = `${out}/latest.json`;
writeFileSync(latest, JSON.stringify({ version: v, notes, pub_date: new Date().toISOString(), platforms: { 'darwin-aarch64': mac, 'darwin-x86_64': mac } }, null, 2));

git('add', 'Cargo.toml', 'Cargo.lock');
git('commit', '-m', `Release ${v}`);
git('tag', '-a', `v${v}`, '-m', notes);
execFileSync('git', ['push', '--follow-tags'], { stdio: 'inherit' });
// a draft until every file is up: the updater only sees published releases
run('gh', ['release', 'create', `v${v}`, '--repo', repo, '--verify-tag', '--draft', '--title', `Recipes ${v}`, '--notes', notes, dmg, bundle, latest], { stdio: 'inherit' });
run('gh', ['release', 'edit', `v${v}`, '--repo', repo, '--draft=false', '--latest'], { stdio: 'inherit' });
console.log(`\nPublished https://github.com/${repo}/releases/tag/v${v}`);
