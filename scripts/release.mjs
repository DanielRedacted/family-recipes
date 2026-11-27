// Usage: node scripts/release.mjs 0.2.0 "What changed, one line per item"
// Builds Recipes.app on this Mac (Apple silicon + Intel), signs the update with the key in ~/.tauri,
// commits the version, tags vX.Y.Z (the tag message becomes the release notes), pushes, and publishes
// a GitHub release with the disk image, the update bundle and latest.json. Installed apps then offer it.
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import { homedir, tmpdir } from 'node:os';

const [v, notes] = process.argv.slice(2);
const die = (msg) => { console.error(msg); process.exit(1); };
if (!/^\d+\.\d+\.\d+$/.test(v || '')) die('usage: node scripts/release.mjs X.Y.Z "release notes"');
if (!notes) die('Add release notes as the second argument; they show up in the app under "What\'s new".');

const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: 'utf8', ...opts });
const git = (...a) => run('git', a).trim();
const key = `${homedir()}/.tauri/family-recipes.key`;
if (!existsSync(key)) die(`No signing key at ${key}. Restore it from your backup; without it installed apps reject updates.`);
if (git('status', '--porcelain', '--untracked-files=no')) die('Commit or stash your changes first.'); // untracked files never go into the release commit
if (git('tag', '-l', `v${v}`)) die(`v${v} already exists.`);

const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const repo = conf.plugins.updater.endpoints[0].match(/github\.com\/([^/]+\/[^/]+)\/releases/)[1];
const cargo = readFileSync('Cargo.toml', 'utf8');
const current = cargo.match(/^version = "([^"]+)"/m)[1];
const newer = (a, b) => { const x = a.split('.').map(Number), y = b.split('.').map(Number); for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i]; return false; };
if (!newer(v, current)) die(`${v} must be newer than the current ${current}, or installed apps won't see it as an update.`);

// Nothing is changed, built or published until the tests pass: installed apps can update themselves.
const check = (label, cmd, args, opts = {}) => {
  console.log(`\n== ${label}`);
  if (spawnSync(cmd, args, { stdio: 'inherit', ...opts }).status !== 0) die(`${label} failed. Nothing was changed or published.`);
};
const recipes = `${homedir()}/Recipes`;
if (!existsSync(recipes)) die(`No recipes at ${recipes}; the release checks run against a copy of them.`);
check('Rust tests', 'cargo', ['test', '-p', 'recipes-core']);
check('The real recipes parse', 'cargo', ['test', '-p', 'recipes-core', '--', '--ignored', '--skip', 'dump_vault'],
  { env: { ...process.env, VAULT: recipes, TECHNIQUES: `${process.cwd()}/../home-recipes-tools/.tools/techniques.txt` } });
check('UI tests', 'node_modules/.bin/vitest', ['run'], { cwd: 'ui' });
check('Type check', 'npm', ['run', 'check'], { cwd: 'ui' });
check('UI build', 'npm', ['run', 'build'], { cwd: 'ui' });
check('Server build', 'cargo', ['build', '--bin', 'recipes-server']);
// the headless server in a browser, desktop and phone width; the built desktop app is checked after the build
{
  const tmp = mkdtempSync(`${tmpdir()}/recipes-release-`);
  check('Copy of the recipes', 'rsync', ['-a', '--exclude', '.git', '--exclude', '.trash', `${recipes}/`, `${tmp}/vault/`]);
  const server = spawn('target/debug/recipes-server', ['--vault', `${tmp}/vault`, '--data', `${tmp}/data`, '--port', '0'], { stdio: ['ignore', 'pipe', 'inherit'] });
  const stop = () => { server.kill(); rmSync(tmp, { recursive: true, force: true }); };
  process.on('exit', stop); // die() exits without unwinding
  const url = await new Promise((resolve) => {
    let out = '';
    const timer = setTimeout(() => resolve(null), 60_000);
    server.stdout.on('data', (d) => { out += d; const m = out.match(/Recipes: (http\S+)/); if (m) { clearTimeout(timer); resolve(m[1]); } });
    server.on('exit', () => { clearTimeout(timer); resolve(null); });
  });
  if (!url) die('Smoke check failed: the server did not start. Nothing was changed or published.');
  check('Smoke check', 'node', ['e2e/smoke.mjs', url], { cwd: 'ui' });
  process.off('exit', stop);
  stop();
}

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

// The built app itself, on a copy of the recipes (RECIPES_SMOKE in src-tauri/src/main.rs): it must start,
// make the sharing certificate and show the folders. This catches what the headless server can't, such
// as 0.1.1's blank window: a second TLS backend that only the desktop app compiles in.
// ponytail: runs this Mac's half of the universal app only; the other architecture is not started
const built = `${process.cwd()}/${out}/macos/Recipes.app`;
console.log('\n== The built app');
const smokeDir = mkdtempSync(`${tmpdir()}/recipes-release-app-`);
const appOk = spawnSync('rsync', ['-a', '--exclude', '.git', '--exclude', '.trash', `${recipes}/`, `${smokeDir}/vault/`], { stdio: 'inherit' }).status === 0
  // without it, an earlier run that crashed or was stopped makes macOS ask (in a dialog nobody answers)
  // whether to reopen its windows, and the check waits out its time
  && spawnSync(`${built}/Contents/MacOS/${readdirSync(`${built}/Contents/MacOS`)[0]}`, ['-ApplePersistenceIgnoreState', 'YES'],
    { stdio: 'inherit', timeout: 120_000, env: { ...process.env, RECIPES_SMOKE: smokeDir, RUST_LOG: 'error' } }).status === 0;
rmSync(smokeDir, { recursive: true, force: true });

// the build's own Recipes.app (and the disk images mounted while packaging) would otherwise show up
// next to the installed app in Launchpad, Spotlight and Open With
const lsregister = '/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister';
const dump = spawnSync(lsregister, ['-dump'], { encoding: 'utf8', maxBuffer: 1 << 28 }).stdout ?? '';
for (const p of new Set([built, ...(dump.match(/\/Volumes\/dmg\.\w+\/Recipes\.app/g) ?? [])])) spawnSync(lsregister, ['-u', p]);
rmSync(built, { recursive: true, force: true });
if (!appOk) {
  git('checkout', '--', 'Cargo.toml', 'Cargo.lock');
  die('The built app failed its check; the version was put back. Nothing was committed or published.');
}
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
