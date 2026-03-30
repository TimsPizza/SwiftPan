# 媒体缩略图方案（不内置 ffmpeg）

目标：在不打包 ffmpeg 的前提下，实现稳定的媒体（图片/视频）缩略图生成与上传，尽量复用系统能力，最少依赖，跨平台可落地。

本方案延续现有上传逻辑：本地生成 `thumbnail_<源文件名>.jpg`；主文件上传成功后，后台异步上传远端 `thumbnail_<对象 key>.jpg`（已有逻辑）。

---

## 设计原则

- 不内置第三方大组件（ffmpeg）。
- 能力尽量使用平台原生的缩略图/解码接口；不可用时降级。
- 图片在后端本地用纯 Rust 处理；视频优先走系统 API；最后再走前端 WebView 编解码回退。
- 失败不影响主流程；只做 best-effort。

## 平台能力矩阵

- macOS（优先）
  - QuickLookThumbnailing: `QLThumbnailGenerator` 可为多种文件（含视频）产缩略图。
  - 备选：AVFoundation `AVAssetImageGenerator` 抽首帧。
  - 集成方式：新增极小的 Swift/ObjC 桥接（不引入外部库，只链接系统框架）。

- Windows
  - Shell 缩略图：`IShellItemImageFactory`/`IThumbnailProvider` 可生成文件缩略图（含视频/PDF/Office）。
  - 图像编码：WIC（Windows Imaging Component）输出 JPEG。
  - 集成方式：Rust `windows` crate 直接调用 Win32/WinRT API。

- Linux（多发行版差异大）
  - 尝试系统缩略图器（如 `gdk-pixbuf-thumbnailer`）若存在则调用；若缺失则跳过视频缩略图，仅做图片。
  - 不打包任何外部编解码器。

- Android（你项目已有 Android 模块）
  - `MediaMetadataRetriever.getFrameAtTime()` 抽帧；用 Android API 即可。

- 渲染层兜底（跨平台）
  - 对于常见视频容器/编码，WebView（WebKit/WebView2）通常能解码。可在前端用 `<video>` + `<canvas>` 截帧，`canvas.toBlob("image/jpeg")` 得到 JPEG，再通过 Tauri 命令回传给后端保存。
  - 注意：编解码可用性取决于系统安装的解码器；失败则记录日志并放弃。

---

## 后端统一入口

新增 `thumbnail` 模块，导出：

```rust
pub async fn ensure_local_thumbnail(src_path: &str, max_px: u32) -> SpResult<Option<String>>
```

行为：

- 若同目录已存在 `thumbnail_<basename>.jpg` 且 mtime >= 源文件，则直接返回路径。
- 根据扩展名先区分图片/视频：
  - 图片：用 `image` crate 解码 + 等比缩放到 `max_px`，写 JPEG 质量 80。
  - 视频：按平台顺序尝试：
    1. 原生系统缩略图 API（macOS/Windows/Android；Linux 试 gdk-pixbuf-thumbnailer）。
    2. 发送事件给渲染层做 Web 截帧兜底（可开关）。
- 成功则返回 Some(本地缩略图路径)；失败或不支持返回 Ok(None)。

### 图片（纯 Rust）

- 依赖：`image`（启用 jpeg/png/webp）+ `exif`（处理旋转）。
- 处理：读取、按 EXIF 方向旋转、最长边 `max_px` 缩放、JPEG 80 写出。

### 视频（平台实现）

- macOS：
  - 使用 `QLThumbnailGenerator` 生成指定 `CGSize(max_px, max_px)` 的缩略图，输出 CGImage，再编码成 JPEG 写文件。
  - 集成：在 `src-tauri/` 下加一个小型 Swift/ObjC 文件，暴露 C ABI：
    ```c
    bool sp_generate_thumb_macos(const char* c_path, int max_px, /* out */ struct SpThumbBuf* out);
    ```
    通过 `build.rs` 使用 `cc` 或 `swiftc` 编译，并在 Cargo 配置里链接 `QuickLookThumbnailing`, `CoreGraphics`, `AppKit`。

- Windows：
  - 通过 `windows` crate：
    - `SHCreateItemFromParsingName` 获取 `IShellItem`。
    - `IShellItemImageFactory::GetImage` 生成 `HBITMAP`（大小 max_px）。
    - 用 WIC 将 `HBITMAP` 转为 JPEG 字节并落盘。

- Linux：
  - 查找可执行 `gdk-pixbuf-thumbnailer`（或同类 XDG 缩略图器）：
    - 若存在：`gdk-pixbuf-thumbnailer -s <max_px> <src> <dst>`，超时 5s。
    - 若不存在或失败：返回 None。

- Android：
  - 用 `MediaMetadataRetriever` 抽帧并压缩 JPEG；你已有 Android 后端模块，可在该模块中加入 JNI/插件方法，并在 Rust 侧通过 tauri 插件或 JNI 调用。

### 渲染层兜底（可选、默认关闭或延迟触发）

- 后端发事件 `sp://thumbnail_request` 携带 `{ id, src_path, max_px }`。
- 前端：读取本地文件为 Blob（通过后端提供的只读通道或 allowlist 路径），创建 `<video>` 播放到 1s，`drawImage` 到 `<canvas>`，`toBlob('image/jpeg', 0.8)`，把字节流通过 `invoke` 传回 `sp://thumbnail_response`（附 id）。
- 后端等待 one-shot，收到后写入本地缩略图路径。

---

## 与现有上传流程的集成

在 `upload::run_upload` 中，读取到 `settings.upload_thumbnail` 后先调用：

```rust
let _ = crate::thumbnail::ensure_local_thumbnail(&params.source_path, 512).await;
```

随后原有的“查找本地 thumbnail\_\*.jpg 并在主文件完成后异步上传”的逻辑即可命中。

流式上传仍保持默认不自动生成；若前端能拿到本地路径，可通过新增命令 `backend_generate_and_upload_thumbnail` 主动触发一次（可选）。

---

## 依赖与打包影响

- 图片：`image` + `exif`（纯 Rust，小体积）。
- macOS/Windows/Android：仅链接系统框架/API，不引入第三方大库。
- Linux：不强依赖外部工具；发现系统缩略图器则利用，否则跳过视频缩略图。
- 不内置 ffmpeg。

---

## 错误与降级策略

- 任一步失败：只记录日志并返回 None，不影响主上传。
- 生成并发：可通过一个有界 mpsc 队列限制（例如同时 2 个）。
- 安全：只处理本地可读路径；禁止网络路径；输出文件统一命名到与源文件同目录。

---

## 实施分解

1. 新建 `thumbnail` 模块（Rust），先实现图片路径与总入口，落盘命名规则与缓存判断。
2. macOS 实现（优先）：添加 Swift/ObjC 桥接与 build.rs；打通到 Rust。
3. Windows 实现：windows + WIC 管线。
4. Linux 探测 `gdk-pixbuf-thumbnailer`，实现外部调用与超时；缺失则返回 None。
5. （可选）前端兜底：事件协议与一次往返实现。
6. 在 `upload::run_upload` 插入入口调用；回归测试各平台。

---

## 代码骨架（Rust 端）

```rust
// src-tauri/src/thumbnail/mod.rs

pub async fn ensure_local_thumbnail(src: &str, max_px: u32) -> SpResult<Option<String>> {
    use std::path::{Path, PathBuf};
    let p = Path::new(src);
    let Some(name) = p.file_name().and_then(|s| s.to_str()) else { return Ok(None) };
    let Some(dir) = p.parent() else { return Ok(None) };
    let dst = dir.join(format!("thumbnail_{}.jpg", name));

    // 缓存命中
    if let (Ok(m_src), Ok(m_dst)) = (tokio::fs::metadata(p).await, tokio::fs::metadata(&dst).await) {
        if let (Ok(t_src), Ok(t_dst)) = (m_src.modified(), m_dst.modified()) {
            if t_dst >= t_src { return Ok(Some(dst.to_string_lossy().to_string())); }
        }
    }

    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let is_img = matches!(ext.as_str(), "jpg"|"jpeg"|"png"|"webp");
    let is_vid = matches!(ext.as_str(), "mp4"|"mov"|"mkv"|"avi"|"webm");

    if is_img {
        return gen_image_thumb(p, &dst, max_px).await.map(|_| Some(dst.to_string_lossy().to_string()));
    }
    if is_vid {
        // 平台优先
        #[cfg(target_os = "macos")]
        if gen_video_thumb_macos(p, &dst, max_px).await.is_ok() {
            return Ok(Some(dst.to_string_lossy().to_string()));
        }
        #[cfg(target_os = "windows")]
        if gen_video_thumb_windows(p, &dst, max_px).await.is_ok() {
            return Ok(Some(dst.to_string_lossy().to_string()));
        }
        #[cfg(target_os = "linux")]
        if gen_video_thumb_linux_external(p, &dst, max_px).await.is_ok() {
            return Ok(Some(dst.to_string_lossy().to_string()));
        }
        // 兜底：发事件给前端（可选，默认关闭）
        if maybe_canvas_fallback(p, &dst, max_px).await.is_ok() {
            return Ok(Some(dst.to_string_lossy().to_string()));
        }
    }
    Ok(None)
}
```

> 注：各平台函数在本方案中给出实现思路与 API，对应细节需按平台分别落地；不需要引入 ffmpeg。

---

## 验收标准

- 图片/视频在 macOS、Windows 能生成缩略图；Linux 至少图片可生成，视频视系统环境而定；Android 用系统 API。
- 生成文件命名与现有上传逻辑兼容，R2 可见 `thumbnail_<key>.jpg`。
- 失败降级可用，不阻塞主上传。
