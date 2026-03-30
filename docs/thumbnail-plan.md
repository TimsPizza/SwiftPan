# 媒体文件缩略图「提取 + 上传」后端方案

本方案针对 SwiftPan 的 Tauri Rust 后端，补齐“自动生成媒体缩略图并上传到 R2”的能力，与现有上传流程无缝对接。

## 现状小结（从代码确认）

- `settings.upload_thumbnail` 为开关。
- 上传文件（非流式）时，后端会在源文件同目录寻找 `thumbnail_<源文件名>.jpg`，若存在则在主文件上传完成后异步上传到远端 `thumbnail_<对象 key>.jpg`。
- 目前后端不会“生成”缩略图；流式上传模式明确“不支持自动上传缩略图”。

## 目标

- 在本地为常见媒体生成 JPEG 缩略图，再复用现有“本地缩略图文件 -> 异步上传”的逻辑。
- 失败不影响主文件上传（best-effort）。
- 可配置、跨平台、尽量轻依赖。

## 支持范围

- 图片：jpeg/png/webp（优先使用 Rust `image` + `exif` 处理旋转）。
- 视频：mp4/mov/mkv 等（优先通过系统 `ffmpeg` CLI 抽帧；如不可用则跳过视频缩略图）。
- 音频（可选）：提取封面图（id3/mp4 封面）；若无，则可选生成波形图（扩展项，默认不做）。

## 输出要求

- 文件名：
  - 本地：`thumbnail_<源文件名>.jpg`
  - 远端：`thumbnail_<对象 key>.jpg`（沿用现有实现）
- 参数：最长边 512px，等比缩放，JPEG 品质 80，RGB，去除 EXIF。
- 元数据：`content-type: image/jpeg`，不设置额外缓存/ACL（遵循当前 R2 策略）。

## 生成策略与缓存

- 若同目录已存在 `thumbnail_<源文件名>.jpg` 且 mtime >= 源文件 mtime，则跳过生成（命中缓存）。
- 否则尝试生成；失败即放弃（记录日志）。

## 并发与调度

- 生成属于 CPU/IO 密集：
  - 图片走 `tokio::task::spawn_blocking`，避免阻塞运行时。
  - 视频走 `tokio::process::Command` 调用 `ffmpeg`，设置超时（例如 10 秒）。
- 通过一个轻量队列（有界 `mpsc`）限制并发（例如最多 2 个同时生成）。

## 与现有上传流程的集成点

- 在 `upload::run_upload` 开始阶段、读取 `settings.upload_thumbnail` 后：
  1. 先调用 `ensure_local_thumbnail(&params.source_path)` 生成或复用本地缩略图。
  2. 原有的“检测本地缩略图 -> 完成主上传后异步上传”逻辑保持不变（即可直接命中）。
- 流式上传模式：默认仍不自动生成。可提供独立命令：
  - `backend_generate_and_upload_thumbnail({ key, source_path? })`
  - 用于桌面端“本地文件路径已知”的场景，由前端在主文件上传结束后单独触发。

## Rust 模块设计

- 新增模块：`src-tauri/src/thumbnail/mod.rs`
  - `pub async fn ensure_local_thumbnail(src: &str) -> Result<Option<String>, SpError>`
    - 返回 Some(本地缩略图路径) 或 None（不支持/失败）。
  - `fn is_image(path: &Path) -> bool` / `fn is_video(path: &Path) -> bool`
  - `async fn gen_image_thumb(src, dst) -> Result<()>`（spawn_blocking + image/exif）
  - `async fn gen_video_thumb(src, dst) -> Result<()>`（ffmpeg CLI + 超时）
  - 平台扩展（可选）：macOS 可用 `sips` 处理 HEIC；Android 可用 `media3`/`exoplayer`（后续）。

## 关键代码示例

### 1) 图片缩略图（image + exif）

```rust
// Cargo.toml（节选）
// [dependencies]
// image = { version = "0.25", default-features = false, features = ["jpeg", "png", "webp"] }
// exif = "0.7"
// tokio = { version = "1", features = ["rt-multi-thread", "macros", "process"] }

fn load_and_orient(mut bytes: &[u8]) -> anyhow::Result<image::DynamicImage> {
    // 读 EXIF 方向
    let orientation = exif::Reader::new()
        .read_from_container(&mut bytes)
        .ok()
        .and_then(|exif| {
            exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        })
        .unwrap_or(1);
    // 重新读取解码（因上面把 reader 前进了）
    let img = image::load_from_memory(bytes)?;
    let img = match orientation {
        3 => img.rotate180(),
        6 => img.rotate90(),
        8 => img.rotate270(),
        _ => img,
    };
    Ok(img)
}

fn resize_max(img: &image::DynamicImage, max: u32) -> image::DynamicImage {
    let (w, h) = img.dimensions();
    let scale = (max as f32 / w as f32).max(max as f32 / h as f32);
    if scale >= 1.0 { return img.clone(); }
    img.resize(max, max, image::imageops::FilterType::CatmullRom)
}

pub async fn gen_image_thumb(src: &std::path::Path, dst: &std::path::Path) -> SpResult<()> {
    let src = src.to_owned();
    let dst = dst.to_owned();
    tokio::task::spawn_blocking(move || -> SpResult<()> {
        let bytes = std::fs::read(&src).map_err(|e| err_not_retriable(format!("read: {e}")))?;
        let img = load_and_orient(&bytes).map_err(|e| err_not_retriable(format!("decode: {e}")))?;
        let img = resize_max(&img, 512);
        let mut out = vec![];
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
        enc.encode_image(&img).map_err(|e| err_not_retriable(format!("encode: {e}")))?;
        std::fs::write(&dst, out).map_err(|e| err_not_retriable(format!("write: {e}")))?;
        Ok(())
    })
    .await
    .unwrap_or_else(|e| Err(err_not_retriable(format!("join err: {e}"))))
}
```

### 2) 视频缩略图（ffmpeg CLI）

```rust
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub async fn gen_video_thumb(src: &std::path::Path, dst: &std::path::Path) -> SpResult<()> {
    // 取第 1 秒处抽帧，最长边 512，-2 保持偶数避免像素格式问题
    let args = [
        "-y", "-ss", "00:00:01", "-i", src.to_string_lossy().as_ref(),
        "-frames:v", "1", "-vf", "scale=512:-2:force_original_aspect_ratio=decrease",
        "-q:v", "4", dst.to_string_lossy().as_ref()
    ];
    let mut cmd = Command::new("ffmpeg");
    cmd.args(args);
    match timeout(Duration::from_secs(10), cmd.status()).await {
        Ok(Ok(st)) if st.success() => Ok(()),
        Ok(Ok(st)) => Err(err_not_retriable(format!("ffmpeg exit: {st}"))),
        Ok(Err(e)) => Err(err_not_retriable(format!("ffmpeg spawn: {e}"))),
        Err(_) => Err(err_not_retriable("ffmpeg timeout")),
    }
}
```

### 3) 组合入口

```rust
pub async fn ensure_local_thumbnail(src_path: &str) -> SpResult<Option<String>> {
    use std::path::{Path, PathBuf};
    let src = Path::new(src_path);
    let Some(name) = src.file_name().and_then(|s| s.to_str()) else { return Ok(None); };
    let Some(dir) = src.parent() else { return Ok(None); };
    let dst = dir.join(format!("thumbnail_{}.jpg", name));

    // 命中缓存
    if let (Ok(m_src), Ok(m_dst)) = (tokio::fs::metadata(src).await, tokio::fs::metadata(&dst).await) {
        if let (Ok(t_src), Ok(t_dst)) = (m_src.modified(), m_dst.modified()) {
            if t_dst >= t_src { return Ok(Some(dst.to_string_lossy().to_string())); }
        }
    }

    // 分类型生成
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let is_img = matches!(ext.as_str(), "jpg"|"jpeg"|"png"|"webp");
    let is_vid = matches!(ext.as_str(), "mp4"|"mov"|"mkv"|"avi"|"webm");

    let ret = if is_img {
        gen_image_thumb(src, &dst).await.map(|_| Some(dst))
    } else if is_vid {
        gen_video_thumb(src, &dst).await.map(|_| Some(dst))
    } else { Ok(None) };

    ret.map(|opt| opt.map(|p| p.to_string_lossy().to_string()))
}
```

## API/命令约定

- 新增后端函数：`thumbnail::ensure_local_thumbnail(src_path) -> Option<String>`
- 集成点：`upload::run_upload` 中，在读取设置后调用：
  ```rust
  if settings::get().upload_thumbnail {
      let _ = crate::thumbnail::ensure_local_thumbnail(&params.source_path).await;
  }
  ```
  随后原有的 `maybe_thumb_local` 检测逻辑即可命中并触发异步上传。
- 可选新增 Tauri 命令（用于流式上传/安卓）：
  - `backend_generate_and_upload_thumbnail(key: String, source_path: String)`
  - 仅当 `source_path` 在本地可读时执行；成功则直接调用现有 R2 上传。

## 依赖与体积

- 新增依赖：`image`, `exif`, `tokio/process`。
- 视频缩略图依赖系统 `ffmpeg` 可执行文件：
  - macOS 推荐：`brew install ffmpeg`
  - Windows/Linux：自行安装或在设置中关闭视频缩略图。
- 不强制内嵌 FFmpeg 静态库，避免体积与许可复杂度。

## 边界情况与降级

- HEIC/HEIF：`image` 默认不支持；macOS 可用 `sips` 转 JPEG（可后续补平台分支）。
- 超大图片：解码占用内存较高；可先用 `image::io::Reader::with_guessed_format` + `limits` 控制（可选）。
- 视频损坏/编码异常：ffmpeg 失败则跳过。
- 权限不足/只读盘：生成失败直接跳过。
- 流式上传：保持不自动生成；提供独立命令供前端按需调用。

## 性能与资源

- 限制并发（如队列深度 2）防止拖慢主上传。
- 每个视频生成设定 10s 超时；图片生成单张通常 <100ms（视素材）。

## 上线步骤

1. 新增 `thumbnail` 模块与依赖，编译验证。
2. 在 `upload::run_upload` 中插入 `ensure_local_thumbnail` 调用。
3. 手动测试：
   - 图片、视频、音频（若实现）各 2～3 个样本。
   - 检查远端是否出现 `thumbnail_<key>.jpg`。
4. 文档与设置面板说明补充：视频缩略图需要本地 `ffmpeg`。

## 后续可选项

- HEIC 支持（平台原生或 libheif）。
- 音频波形图生成。
- 统一的缩略图 Key 布局（如 `.thumbs/<key>.jpg`），需要前后端一起改。
- 缩略图按需生成（访问时 Serverless 生成并缓存），适合重度 Web 端，但与当前“本地生成并上传”的模式不同。
