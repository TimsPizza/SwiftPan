#!/usr/bin/env node
import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");

function log(msg) {
  console.log(msg);
}
function fail(msg, code = 1) {
  console.error(msg);
  process.exit(code);
}

function readJson(p) {
  return JSON.parse(fs.readFileSync(p, "utf8"));
}
function writeJson(p, obj) {
  fs.writeFileSync(p, JSON.stringify(obj, null, 2) + "\n");
}

function readText(p) {
  return fs.readFileSync(p, "utf8");
}
function writeText(p, s) {
  fs.writeFileSync(p, s);
}

function exec(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: "inherit", ...opts });
  if (res.status !== 0) {
    fail(`Command failed: ${cmd} ${args.join(" ")}`);
  }
}

function tryExec(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: "inherit", ...opts });
  return res.status === 0;
}

const args = process.argv.slice(2);

// Robust argv parsing supporting: "--key=value", "--key value", and positional type
let type;
let dryRun = false;
let noPush = false;
let dispatch = false;
let target = "android"; // build target for workflow_dispatch

for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === "--type") {
    if (i + 1 >= args.length) fail("Missing value for --type");
    type = args[++i];
    continue;
  }
  if (a.startsWith("--type=")) {
    type = a.slice("--type=".length);
    continue;
  }
  if (a === "--dry-run") {
    dryRun = true;
    continue;
  }
  if (a === "--no-push") {
    noPush = true;
    continue;
  }
  if (a === "--dispatch") {
    dispatch = true;
    continue;
  }
  if (a === "--target") {
    if (i + 1 >= args.length) fail("Missing value for --target");
    target = args[++i];
    continue;
  }
  if (a.startsWith("--target=")) {
    target = a.slice("--target=".length);
    continue;
  }
  if (
    !a.startsWith("--") &&
    ["beta", "patch", "minor", "major", "release"].includes(a) &&
    !type
  ) {
    type = a;
    continue;
  }
}

if (!type) {
  console.log(
    "Usage: pnpm release --type <beta|patch|minor|major|release> [--dry-run] [--no-push] [--dispatch] [--target <all|android|ubuntu|macos-arm|windows>]",
  );
  process.exit(1);
}

const allowedTargets = ["all", "android", "ubuntu", "macos-arm", "windows"];
if (!allowedTargets.includes(target)) {
  fail(`Invalid --target '${target}'. Allowed: ${allowedTargets.join(", ")}`);
}

// Semver helpers
function parse(v) {
  const m = v.trim().match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/);
  if (!m) fail(`Invalid semver: ${v}`);
  return { major: +m[1], minor: +m[2], patch: +m[3], pre: m[4] || null };
}

function fmt({ major, minor, patch, pre }) {
  return `${major}.${minor}.${patch}` + (pre ? `-${pre}` : "");
}

function toMsiVersion(version) {
  const { major, minor, patch, pre } = parse(version);
  if (major > 255 || minor > 255) {
    fail(
      `MSI version segments ${major}.${minor} exceed Windows limit of 255 for the first two components`,
    );
  }
  if (patch > 65535) {
    fail(`MSI patch segment ${patch} exceeds Windows limit of 65535`);
  }

  const segments = [major, minor, patch];

  if (pre) {
    const numericPart = pre
      .split(/[.-]/)
      .map((part) => part.trim())
      .find((part) => /^\d+$/.test(part));

    const buildNumber = numericPart ? Number(numericPart) : 0;

    if (buildNumber > 65535) {
      fail(`MSI build segment ${buildNumber} exceeds Windows limit of 65535`);
    }

    segments.push(buildNumber);
  }

  return segments.join(".");
}

function sanitizeVersion({ major, minor, patch }) {
  return fmt({ major, minor, patch, pre: null });
}

function bumpPatch({ major, minor, patch }) {
  return fmt({ major, minor, patch: patch + 1, pre: null });
}

function bumpMinor({ major, minor }) {
  return fmt({ major, minor: minor + 1, patch: 0, pre: null });
}

function bumpMajor({ major }) {
  return fmt({ major: major + 1, minor: 0, patch: 0, pre: null });
}

function escapeForRegex(str) {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function gitTagExists(tag) {
  const res = spawnSync("git", ["rev-parse", "--verify", tag], {
    cwd: repoRoot,
    stdio: "ignore",
  });
  return res.status === 0;
}

function getBetaTagNumbers(baseVersion) {
  const res = spawnSync("git", ["tag", "--list", `v${baseVersion}-beta.*`], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (res.status !== 0) return [];
  const pattern = new RegExp(`^v${escapeForRegex(baseVersion)}-beta\\.(\\d+)$`);
  return res.stdout
    .split(/\r?\n/)
    .map((line) => {
      const m = line.trim().match(pattern);
      return m ? Number(m[1]) : null;
    })
    .filter((n) => Number.isInteger(n));
}

function nextVersion(current, type) {
  const parsed = parse(current);
  const base = {
    major: parsed.major,
    minor: parsed.minor,
    patch: parsed.patch,
  };
  const sanitizedCurrent = sanitizeVersion(base);

  switch (type) {
    case "beta": {
      const currentPreMatch = parsed.pre
        ? parsed.pre.match(/^beta\.(\d+)$/)
        : null;

      let manifestVersion = sanitizedCurrent;

      const releaseTagExists = gitTagExists(`v${sanitizedCurrent}`);
      const betaNumbersForCurrent = getBetaTagNumbers(sanitizedCurrent);

      if (
        !currentPreMatch &&
        !betaNumbersForCurrent.length &&
        releaseTagExists
      ) {
        manifestVersion = bumpPatch(base);
      } else if (
        !currentPreMatch &&
        !betaNumbersForCurrent.length &&
        !releaseTagExists
      ) {
        // No record of this version yet; start by bumping patch so betas track upcoming release.
        manifestVersion = bumpPatch(base);
      }

      const betaNumbers = getBetaTagNumbers(manifestVersion);
      let nextBeta = betaNumbers.length ? Math.max(...betaNumbers) + 1 : 1;

      if (currentPreMatch) {
        nextBeta = Math.max(nextBeta, Number(currentPreMatch[1]) + 1);
      }

      const tagVersion = `${manifestVersion}-beta.${nextBeta}`;
      return { manifestVersion, tagVersion };
    }
    case "release": {
      const manifestVersion = sanitizedCurrent;
      return { manifestVersion, tagVersion: manifestVersion };
    }
    case "patch": {
      const manifestVersion = bumpPatch(base);
      return { manifestVersion, tagVersion: manifestVersion };
    }
    case "minor": {
      const manifestVersion = bumpMinor(base);
      return { manifestVersion, tagVersion: manifestVersion };
    }
    case "major": {
      const manifestVersion = bumpMajor(base);
      return { manifestVersion, tagVersion: manifestVersion };
    }
    default:
      fail(`Unknown type: ${type}`);
  }
}

// Read current versions
const pkgPath = path.join(repoRoot, "package.json");
const tauriConfPath = path.join(repoRoot, "src-tauri", "tauri.conf.json");
const cargoPath = path.join(repoRoot, "src-tauri", "Cargo.toml");

const pkg = readJson(pkgPath);
const current = pkg.version;
const { manifestVersion, tagVersion } = nextVersion(current, type);
const msiVersion = toMsiVersion(tagVersion);

log(`Current manifest: ${current}`);
log(`Next manifest   : ${manifestVersion} (${type})`);
log(`Git tag version : ${tagVersion}`);
log(`Windows MSI     : ${msiVersion}`);

// Update package.json
pkg.version = manifestVersion;

// Update tauri.conf.json
const tauriConf = readJson(tauriConfPath);
tauriConf.version = manifestVersion;
if (!tauriConf.bundle) tauriConf.bundle = {};
if (!tauriConf.bundle.windows) tauriConf.bundle.windows = {};
if (!tauriConf.bundle.windows.wix) tauriConf.bundle.windows.wix = {};
tauriConf.bundle.windows.wix.version = msiVersion;

// Update Cargo.toml [package] version
let cargo = readText(cargoPath);
// Replace the first version = "x" under [package]
const pkgHeaderIdx = cargo.indexOf("[package]");
if (pkgHeaderIdx === -1) fail("src-tauri/Cargo.toml missing [package] section");
const before = cargo.slice(0, pkgHeaderIdx);
let rest = cargo.slice(pkgHeaderIdx);
rest = rest.replace(
  /^(\[package\][\s\S]*?\nversion\s*=\s*")([^"]+)("\s*$)/m,
  (_, a, _v, c) => `${a}${manifestVersion}${c}`,
);
if (!/\nversion\s*=\s*"/.test(rest)) {
  // Fallback: generic first version key
  rest = rest.replace(
    /version\s*=\s*"[^"]+"/,
    `version = "${manifestVersion}"`,
  );
}
cargo = before + rest;

if (dryRun) {
  log("[dry-run] Would write package.json, tauri.conf.json, Cargo.toml");
} else {
  writeJson(pkgPath, pkg);
  writeJson(tauriConfPath, tauriConf);
  writeText(cargoPath, cargo);
}

// Git commit and tag
const tag = `v${tagVersion}`;
if (!dryRun) {
  // Stage only the relevant files
  exec("git", [
    "add",
    "--",
    "package.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
  ]);
  // Create commit only if there are staged changes
  const diffCheck = spawnSync("git", ["diff", "--cached", "--quiet"]);
  if (diffCheck.status !== 0) {
    exec("git", ["commit", "-m", `chore(release): ${tag}`]);
  } else {
    log("No file changes to commit.");
  }

  // Create or move tag
  // If tag exists locally, delete and recreate to current HEAD
  const tagExists = tryExec("git", ["rev-parse", "--verify", tag]);
  if (tagExists) {
    log(`Tag ${tag} exists, updating to current HEAD`);
    exec("git", ["tag", "-d", tag]);
  }
  exec("git", ["tag", "-a", tag, "-m", tag]);

  if (!noPush) {
    // Push commit and tag
    exec("git", ["push", "origin", "HEAD"]);
    exec("git", ["push", "origin", tag]);

    // Create/update release branch
    const releaseBranch = `release-${tagVersion}`;
    // Create or update local branch pointing to HEAD
    const branchExists = tryExec("git", [
      "show-ref",
      "--verify",
      `refs/heads/${releaseBranch}`,
    ]);
    if (branchExists) {
      // Move branch to current HEAD (fast-forward or reset) without checkout
      exec("git", ["branch", "-f", releaseBranch, "HEAD"]);
    } else {
      exec("git", ["branch", releaseBranch, "HEAD"]);
    }
    // Push (force-with-lease to keep remote updated safely)
    exec("git", [
      "push",
      "origin",
      `${releaseBranch}:refs/heads/${releaseBranch}`,
      "--force-with-lease",
    ]);
    log(`Pushed branch ${releaseBranch}`);
  } else {
    log("Skipping git push due to --no-push");
  }
}

if (dispatch) {
  // Optionally dispatch the workflow manually (requires GitHub CLI)
  const hasGh =
    !!spawnSync("bash", ["-lc", "command -v gh >/dev/null 2>&1"]).status ===
    false;
  // The above check isn't reliable with spawnSync + inherit. Try a direct attempt.
  const ok = tryExec("gh", ["--version"]);
  if (!ok) {
    log("gh CLI not found or not authenticated; skipping workflow dispatch.");
  } else {
    log("Dispatching GitHub Actions workflow: Build Desktop + Android");
    const dispatchArgs = [
      "workflow",
      "run",
      "Build Desktop + Android",
      "-f",
      `publish_release=true`,
      "-f",
      `target=${target}`,
    ];
    log(`gh ${dispatchArgs.join(" ")}`);
    tryExec("gh", dispatchArgs);
  }
}

log("Done.");
