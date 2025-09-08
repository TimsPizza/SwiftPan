# Set up Cloudflare R2 for SwiftPan

This guide walks you from zero to a working R2 bucket SwiftPan can use.

## 1) Create a Cloudflare account

- Go to https://dash.cloudflare.com/ and sign up (free plan is fine).

## 2) Enable R2 and create a bucket

- In the dashboard, open R2 → Buckets → Create bucket.
- Name: anything you like (e.g., `swiftpan`), keep region as default (R2 is global).

## 3) Create S3 API credentials

SwiftPan talks to R2 via the S3 API.

- R2 → S3 API Tokens → Create API token.
- Save these values somewhere safe:
  - Account ID (also shown at the top right in dashboard)
  - Access Key ID
  - Secret Access Key
- S3 endpoint format:
  - `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`
  - Example: `https://1234567890abcdef.r2.cloudflarestorage.com`
  - Region: many SDKs use `auto`; SwiftPan accepts the endpoint directly.

## 4) Configure CORS (recommended)

If you generate share links or interact via browsers, add a permissive CORS during setup (tighten later).

- In Dashboard (R2 → Buckets → your bucket → Settings → CORS), add a rule:
  - Allowed origins: `*` (or your domain[s])
  - Allowed methods: `GET, PUT, HEAD, POST, DELETE`
  - Allowed headers: `*`
  - Expose headers: `ETag`
  - Max age: `3600`

Example (JSON style used by dashboard UIs):

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

Security tip: once things work, replace `*` with your exact origin(s) and drop unused methods.

## 5) (Optional) Public access strategy

- For simpler public downloads, you can use pre‑signed URLs (recommended) or enable public access for the bucket (where available).
- Pre‑signed URLs avoid exposing your bucket to the world and usually don’t need wide‑open CORS.

## 6) Plug into SwiftPan

Open SwiftPan → Settings and fill in:

- Bucket name
- Account ID
- Access Key ID
- Secret Access Key
- Endpoint: `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`
- Then click “Test connection”.

## Troubleshooting

- 403 / CORS errors: verify the bucket’s CORS rules and your Origin.
- SignatureDoesNotMatch: wrong endpoint or Account ID; ensure you used the exact `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`.
- Clock skew: if requests are signed by local tools, ensure your system time is correct.
