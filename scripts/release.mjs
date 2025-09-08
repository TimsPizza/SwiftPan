#!/usr/bin/env node
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { spawnSync } from 'child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

function log(msg) { console.log(msg); }
function fail(msg, code = 1) { console.error(msg); process.exit(code); }

function readJson(p) { return JSON.parse(fs.readFileSync(p, 'utf8')); }
function writeJson(p, obj) { fs.writeFileSync(p, JSON.stringify(obj, null, 2) + '\n'); }

function readText(p) { return fs.readFileSync(p, 'utf8'); }
function writeText(p, s) { fs.writeFileSync(p, s); }

function exec(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', ...opts });
  if (res.status !== 0) {
    fail(`Command failed: ${cmd} ${args.join(' ')}`);
  }
}

function tryExec(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', ...opts });
  return res.status === 0;
}

const args = process.argv.slice(2);

// Robust argv parsing supporting: "--key=value", "--key value", and positional type
let type;
let dryRun = false;
let noPush = false;
let dispatch = false;

for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--type') {
    if (i + 1 >= args.length) fail('Missing value for --type');
    type = args[++i];
    continue;
  }
  if (a.startsWith('--type=')) {
    type = a.slice('--type='.length);
    continue;
  }
  if (a === '--dry-run') { dryRun = true; continue; }
  if (a === '--no-push') { noPush = true; continue; }
  if (a === '--dispatch') { dispatch = true; continue; }
  if (!a.startsWith('--') && ['beta','patch','minor','major','release'].includes(a) && !type) {
    type = a;
    continue;
  }
}

if (!type) {
  console.log('Usage: pnpm release --type <beta|patch|minor|major|release> [--dry-run] [--no-push] [--dispatch]');
  process.exit(1);
}

// Semver helpers
function parse(v) {
  const m = v.trim().match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/);
  if (!m) fail(`Invalid semver: ${v}`);
  return {
    major: +m[1], minor: +m[2], patch: +m[3], pre: m[4] || null
  };
}

function fmt({ major, minor, patch, pre }) {
  return `${major}.${minor}.${patch}` + (pre ? `-${pre}` : '');
}

function nextVersion(current, type) {
  const s = parse(current);
  const base = { major: s.major, minor: s.minor, patch: s.patch, pre: null };
  switch (type) {
    case 'beta': {
      if (s.pre && /^beta\.(\d+)$/.test(s.pre)) {
        const n = +s.pre.split('.')[1] + 1;
        return fmt({ ...s, pre: `beta.${n}` });
      } else {
        // start next patch beta
        return fmt({ ...base, patch: base.patch + 1, pre: 'beta.1' });
      }
    }
    case 'release': {
      // finalize current cycle: drop prerelease if present, else keep as-is
      if (s.pre) return fmt({ ...s, pre: null });
      return fmt(s);
    }
    case 'patch': {
      // bump patch and drop pre
      return fmt({ ...base, patch: base.patch + 1 });
    }
    case 'minor': {
      return fmt({ major: base.major, minor: base.minor + 1, patch: 0, pre: null });
    }
    case 'major': {
      return fmt({ major: base.major + 1, minor: 0, patch: 0, pre: null });
    }
    default:
      fail(`Unknown type: ${type}`);
  }
}

// Read current versions
const pkgPath = path.join(repoRoot, 'package.json');
const tauriConfPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
const cargoPath = path.join(repoRoot, 'src-tauri', 'Cargo.toml');

const pkg = readJson(pkgPath);
const current = pkg.version;
const next = nextVersion(current, type);

log(`Current version: ${current}`);
log(`Next version   : ${next} (${type})`);

// Update package.json
pkg.version = next;

// Update tauri.conf.json
const tauriConf = readJson(tauriConfPath);
tauriConf.version = next;

// Update Cargo.toml [package] version
let cargo = readText(cargoPath);
// Replace the first version = "x" under [package]
const pkgHeaderIdx = cargo.indexOf('[package]');
if (pkgHeaderIdx === -1) fail('src-tauri/Cargo.toml missing [package] section');
const before = cargo.slice(0, pkgHeaderIdx);
let rest = cargo.slice(pkgHeaderIdx);
rest = rest.replace(/^(\[package\][\s\S]*?\nversion\s*=\s*")([^"]+)("\s*$)/m, (_, a, _v, c) => `${a}${next}${c}`);
if (!/\nversion\s*=\s*"/.test(rest)) {
  // Fallback: generic first version key
  rest = rest.replace(/version\s*=\s*"[^"]+"/, `version = "${next}"`);
}
cargo = before + rest;

if (dryRun) {
  log('[dry-run] Would write package.json, tauri.conf.json, Cargo.toml');
} else {
  writeJson(pkgPath, pkg);
  writeJson(tauriConfPath, tauriConf);
  writeText(cargoPath, cargo);
}

// Git commit and tag
const tag = `v${next}`;
if (!dryRun) {
  // Stage only the relevant files
  exec('git', ['add', '--', 'package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml']);
  // Create commit only if there are staged changes
  const diffCheck = spawnSync('git', ['diff', '--cached', '--quiet']);
  if (diffCheck.status !== 0) {
    exec('git', ['commit', '-m', `chore(release): ${tag}`]);
  } else {
    log('No file changes to commit.');
  }

  // Create or move tag
  // If tag exists locally, delete and recreate to current HEAD
  const tagExists = tryExec('git', ['rev-parse', '--verify', tag]);
  if (tagExists) {
    log(`Tag ${tag} exists, updating to current HEAD`);
    exec('git', ['tag', '-d', tag]);
  }
  exec('git', ['tag', '-a', tag, '-m', tag]);

  if (!noPush) {
    // Push commit and tag
    exec('git', ['push', 'origin', 'HEAD']);
    exec('git', ['push', 'origin', tag]);
  } else {
    log('Skipping git push due to --no-push');
  }
}

if (dispatch) {
  // Optionally dispatch the workflow manually (requires GitHub CLI)
  const hasGh = !!spawnSync('bash', ['-lc', 'command -v gh >/dev/null 2>&1']).status === false;
  // The above check isn't reliable with spawnSync + inherit. Try a direct attempt.
  const ok = tryExec('gh', ['--version']);
  if (!ok) {
    log('gh CLI not found or not authenticated; skipping workflow dispatch.');
  } else {
    log('Dispatching GitHub Actions workflow: Build Desktop + Android');
    tryExec('gh', ['workflow', 'run', 'Build Desktop + Android', '-f', 'publish_release=true']);
  }
}

log('Done.');
