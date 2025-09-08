<div align="left">

# SwiftPan — Make the best use of Cloudflare R2

Fast, private, cross‑platform file manager powered by Cloudflare R2.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![CI](https://github.com/TimsPizza/SwiftPan/actions/workflows/release.yml/badge.svg)](https://github.com/TimsPizza/SwiftPan/actions/workflows/release.yml)
<span>&nbsp;</span>
[![Release](https://img.shields.io/github/v/release/TimsPizza/SwiftPan?display_name=tag&sort=semver)](https://github.com/TimsPizza/SwiftPan/releases)
<span>&nbsp;</span>
![Apple Silicon](https://img.shields.io/badge/Apple%20Silicon-brightgreen)

<div>
<span>Languages: English | <a href="./docs/README.zh-CN.md">简体中文</a></span>
</div>
</div>

---

## Available on

<table>
  <tr>
    <td align="center" width="50%">
      <br/>
      <img src="https://raw.githubusercontent.com/TimsPizza/blob/swiftpan/swiftpan/feat-file-list.png" alt="Desktop — Files" width="88%" />
      <span>Desktop</span>
    </td>
    <td align="center" width="50%">
      <br/>
      <div>
      <img src="https://raw.githubusercontent.com/TimsPizza/blob/swiftpan/swiftpan/feat-mobile-filelist.png" alt="Mobile — File List" width="36%" />
      <img src="https://raw.githubusercontent.com/TimsPizza/blob/swiftpan/swiftpan/feat-mobile-sidebar.png" alt="Mobile — Sidebar & Theme" width="36%" />
      </div>
      <span>Android</span>
    </td>

  </tr>
</table>

Supported: Windows / macOS (x86 WIP) / Linux; Android; iOS (WIP)

---

## What is SwiftPan

SwiftPan is a cross‑platform file manager for Cloudflare R2 (Desktop and Android). It provides fast browsing and search, batch download/delete/share, resumable transfers with pause/resume, and one‑click time‑limited share links. A built‑in Usage & Cost panel shows monthly upload/download/storage trends and cost estimates. Your credentials never leave your device; the app has no cloud backend.

## What is Cloudflare R2

Cloudflare R2 is an S3‑compatible object storage service. It offers high performance, zero egress fees, and a generous free tier (storage up to 10 GB), making it an ideal personal cloud drive backend.

## Features

### Transfers & Management

- Batch operations: download, delete, and share.
- Robust transfers: pause/resume/cancel with resume support.

### Files & Sharing

- File browser: search, filter, sort, paginate.
- Share links: single or bulk, time‑limited.

### Usage & Cost

- Monthly trends for upload/download/storage.
- Cost estimates including free tier status.

### UI/UX

- Dark mode and mobile‑friendly layouts.

> Note: Share links are publicly accessible. Use short expirations and be mindful of possible costs.

---

## Install

### Prebuilt binaries (recommended)

- Grab installers from Releases: Windows / macOS / Linux, plus Android APK/AAB.

### Build from source (optional)

With Tauri and Node set up: `pnpm install && pnpm build && pnpm tauri build`.

---

## Get started

1. In Settings, enter your R2 endpoint and credentials (Bucket, etc.).
2. Click “Test connection”.
3. Upload from the Files page.
4. Pick a download location when saving; create share links when needed.

Need help setting up R2? See: [Set up Cloudflare R2](./docs/setup-r2.md)

---

## Privacy & Security

- Credentials are stored locally, encrypted with device keys (never uploaded).
- No cloud backend, no telemetry; only talks to your configured R2.
- Share links are public; use short expirations and watch costs.

---

## FAQ

**Q: R2 only?** Primarily R2. Other S3‑compatible services may work but are untested.

**Q: Connection fails?** Check credentials, network, and your bucket CORS.

**Q: Is my data safe?** Credentials stay local and encrypted; nothing is collected.

---

## Roadmap

- Optional thumbnail generation on upload.
- More platforms and UX polish (macOS x86, iOS).
- Mobile performance improvements.
- Align Android folder picker with desktop (depends on Tauri plugin).

---

## Build & Contribute

Contributions welcome via Issues and PRs.

---

## License

MIT License — see `LICENSE`.
