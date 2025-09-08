# 配置 Cloudflare R2（供 SwiftPan 使用）

这份指南从零开始，带你完成 R2 账号创建、开通、建桶与 CORS 设置。

## 1）注册 Cloudflare 账号

- 访问 https://dash.cloudflare.com/ 注册（免费版即可）。

## 2）开通 R2 并创建 Bucket

- 在控制台进入 R2 → Buckets → Create bucket。
- Bucket 名称任意（例如 `swiftpan`），区域保持默认（R2 为全局）。

## 3）创建 S3 API 凭证

SwiftPan 通过 S3 API 访问 R2。

- R2 → S3 API Tokens → Create API token。
- 保存以下信息：
  - Account ID（也显示在控制台右上角）
  - Access Key ID
  - Secret Access Key
- S3 端点格式：
  - `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`
  - 示例：`https://1234567890abcdef.r2.cloudflarestorage.com`
  - 区域通常使用 `auto`；SwiftPan 直接使用端点。

## 4）配置 CORS（推荐）

如果你会用分享链接或在浏览器交互，先用相对宽松的 CORS 保证可用（后续再收紧）。

- 控制台（R2 → Buckets → 你的 bucket → Settings → CORS）添加规则：
  - Allowed origins：`*`（或你的域名）
  - Allowed methods：`GET, PUT, HEAD, POST, DELETE`
  - Allowed headers：`*`
  - Expose headers：`ETag`
  - Max age：`3600`

示例（Dashboard 常用 JSON 结构）：

```json
[
  {
    "AllowedOrigins": ["*"],
    "AllowedMethods": ["GET", "POST", "PUT", "DELETE", "HEAD"],
    "AllowedHeaders": [
      "Content-Type",
      "Content-Length",
      "Authorization",
      "X-Amz-Date",
      "X-Amz-Content-Sha256",
      "X-Amz-Security-Token",
      "x-amz-checksum-crc32",
      "x-amz-sdk-checksum-algorithm"
    ],
    "ExposeHeaders": [
      "ETag",
      "x-amz-checksum-crc64nvme",
      "x-amz-version-id",
      "Content-Length",
      "Date"
    ],
    "MaxAgeSeconds": 3600
  }
]
```

安全提示：验证可用后，把 `*` 换成你的具体域名，并移除不需要的方法。

## 5）（可选）公开访问策略

- 更安全的公开下载方式是使用预签名 URL（推荐），或者在支持的场景下开启公开访问。
- 预签名 URL 不会暴露整个 bucket，通常也不需要过于宽松的 CORS。

## 6）在 SwiftPan 中填写

打开 SwiftPan → Settings，填写：

- Bucket 名称
- Account ID
- Access Key ID
- Secret Access Key
- Endpoint：`https://<ACCOUNT_ID>.r2.cloudflarestorage.com`
- 然后点击“测试连接”。

## 故障排查

- 403 / CORS 报错：检查 bucket 的 CORS 规则与你的 Origin。
- SignatureDoesNotMatch：端点或 Account ID 错误；确保使用了 `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`。
- 时间偏差：若本地工具参与签名，确认系统时间准确。
