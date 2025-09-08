<div align="left">

# SwiftPan — Cloudflare R2网盘方案

不限速、隐私安全的个人网盘，基于Cloudflare R2

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](../LICENSE)
[![CI](https://github.com/TimsPizza/SwiftPan/actions/workflows/release.yml/badge.svg)](https://github.com/TimsPizza/SwiftPan/actions/workflows/release.yml)
<span>&nbsp;</span>
[![Release](https://img.shields.io/github/v/release/TimsPizza/SwiftPan?display_name=tag&sort=semver)](https://github.com/TimsPizza/SwiftPan/releases)
<span>&nbsp;</span>
![Apple Silicon](https://img.shields.io/badge/Apple%20Silicon-brightgreen)

<div>
<span>语言: 简体中文 | <a href="../README.md">English</a></span>
</div>
</div>

---

## 可用平台

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

支持平台：Windows / macOS (x86 WIP) / Linux；Android; IOS (WIP)

---

## 什么是 SwiftPan

SwiftPan 是一个面向 Cloudflare R2 的跨平台文件管家（桌面与 Android）。它提供文件浏览与搜索、批量下载/删除/分享，任务可暂停/恢复并支持断点续传；可一键生成限时分享链接。内置用量与费用面板，按月展示上传/下载/存储的趋势与成本预估。您的数据完全保密，该应用不会上传任何您的R2凭证信息到云端。

## 什么是 Cloudflare R2

Cloudflare R2是一个基于S3对象存储的云服务。由于它提供极快的下载上传速度和0流量费以及高达10GB的免费存储，因此是一个理想的个人网盘存储方案。

## 功能一览

### 传输与管理

- 批量操作：支持批量下载、删除、分享。
- 状态恢复 & 断点续传：任务可暂停/恢复/取消。

### 文件与分享

- 文件浏览：搜索、筛选、排序、分页，定位更快。
- 分享链接：单个或批量生成限时链接，复制即刻分享。

### 用量与费用

- 月度用量：上传/下载/存储的趋势与汇总。
- 费用估算：展示免费额度与本月预计成本。

### 界面体验

- 深色模式与移动端优化，顺滑好用。

> 注：分享链接为公开可访问，请谨慎外发并设置较短有效期（无法被系统追踪，且可能产生费用）。

---

## 安装

### 预编译版本（推荐）

- 前往 Releases 下载：Windows / macOS / Linux 安装包，以及 Android 安装包。

### 从源码构建（可选）

有一定开发经验的用户可参考 Tauri 文档进行构建：`pnpm install && pnpm build && pnpm tauri build`。

---

## 快速上手

1. 在 Settings 中填入 R2 的地址和密钥信息（Bucket 名称等）。
2. 点击“测试连接”。
3. 在 Files 页面上传文件。
4. 下载时选择保存位置；需要分享时一键生成链接。

需要帮助配置 R2？请看：[配置 Cloudflare R2 教程](./setup-r2.zh-CN.md)

---

## 隐私与安全

- 凭证仅保存在本地应用数据目录中，使用设备密钥加密保存（不上传、不外发）。
- 应用不含云端后端，不做行为遥测；仅与配置的 R2 交互。
- 分享链接公开可达，请按需设置短时效，注意可能产生费用。

---

## 常见问题（FAQ）

**Q: 仅支持 R2 吗？** 主要面向 Cloudflare R2，其他兼容 S3 的服务**理论上**也可使用（未经测试）。

**Q: 连接失败怎么办？** 请检查密钥信息是否正确、网络是否可用、R2 bucket的CORS是否配置正确？

**Q: 我的数据是否安全？** 凭证只保存在本地并加密，应用不会上传或收集你的任何数据。

---

## Roadmap

- 上传时自动生成缩略图（对用户作为可选项）。
- 更多平台与体验优化（macos x86, IOS）。
- 移动端性能优化
- 重构并保证移动端下载folder picker和桌面端一致性（依赖于tauri plugin）

---

## 构建与贡献

欢迎提交 Issue 与 PR 来改进体验与稳定性。

---

## 许可

MIT License — 见 `LICENSE`。
