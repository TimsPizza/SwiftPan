// Bridge types shared with Tauri commands

export type SpError = {
  kind:
    | "Cancelled"
    | "RetryableNet"
    | "RetryableAuth"
    | "NotRetriable"
    | "SourceChanged"
    | "DiskFull"
    | "NotImplemented"
    | "TaskExists";
  message: string;
  retry_after_ms?: number;
  context?: Record<string, unknown>;
  at: number;
};

export type R2Config = {
  endpoint: string;
  access_key_id: string;
  secret_access_key: string;
  bucket: string;
  region?: string;
};
export type BackendState = {
  is_unlocked: boolean;
  unlock_deadline_ms?: number;
  device_id: string;
  is_credential_completed: boolean;
  is_credential_valid: boolean;
};
export type CredentialBundle = {
  r2: R2Config;
};

export type NewUploadParams = {
  key: string;
  source_path: string;
  part_size: number;
  content_type?: string;
  content_disposition?: string;
};
export type UploadStatus = {
  transfer_id: string;
  key: string;
  lifecycle_state: TransferLifecycle;
  phase?: TransferPhase;
  bytes_total: number;
  bytes_done: number;
  parts_completed: number;
  rate_bps: number;
  eta_ms?: number;
  last_error?: SpError;
};
export type UploadEvent =
  | { type: "Started"; transfer_id: string }
  | {
      type: "PartProgress";
      transfer_id: string;
      progress: { part_number: number; bytes_transferred: number };
    }
  | { type: "PartDone"; transfer_id: string; part_number: number; etag: string }
  | { type: "Paused"; transfer_id: string }
  | { type: "Resumed"; transfer_id: string }
  | { type: "Cancelling"; transfer_id: string }
  | { type: "Completed"; transfer_id: string }
  | { type: "Failed"; transfer_id: string; error: SpError }
  | { type: "Cancelled"; transfer_id: string };

export type NewDownloadParams = {
  key: string;
  dest_path?: string;
  chunk_size: number;
  expected_etag?: string;
  android_tree_uri?: string;
  android_relative_path?: string;
  mime?: string;
};
export type TransferLifecycle =
  | "queued"
  | "running"
  | "paused"
  | "cancelling"
  | "completed"
  | "failed"
  | "cancelled";
export type TransferPhase =
  | "preparing_source"
  | "uploading_remote"
  | "finalizing_remote"
  | "preparing_target"
  | "downloading_remote"
  | "materializing_target"
  | "cleaning_up";
export type DownloadStatus = {
  transfer_id: string;
  key: string;
  lifecycle_state: TransferLifecycle;
  phase?: TransferPhase;
  bytes_total?: number;
  bytes_done: number;
  rate_bps: number;
  expected_etag?: string;
  observed_etag?: string;
  temp_path?: string;
  last_error?: SpError;
};
export type TransferSnapshot = {
  transfer_id: string;
  kind: "upload" | "download";
  key: string;
  lifecycle_state: TransferLifecycle;
  phase?: TransferPhase;
  bytes_total?: number;
  bytes_done: number;
  rate_bps: number;
  last_error?: SpError;
  dest_path?: string;
  android_tree_uri?: string;
  android_relative_path?: string;
  temp_path?: string;
  expected_etag?: string;
  observed_etag?: string;
  created_at_ms: number;
  updated_at_ms: number;
};
export type DownloadEvent =
  | { type: "Started"; transfer_id: string }
  | {
      type: "ChunkProgress";
      transfer_id: string;
      progress: { range_start: number; bytes_transferred: number };
    }
  | { type: "ChunkDone"; transfer_id: string; range_start: number; len: number }
  | { type: "Paused"; transfer_id: string }
  | { type: "Resumed"; transfer_id: string }
  | { type: "Cancelling"; transfer_id: string }
  | { type: "Completed"; transfer_id: string }
  | { type: "Failed"; transfer_id: string; error: SpError }
  | { type: "Cancelled"; transfer_id: string }
  | { type: "SourceChanged"; transfer_id: string };

export type ShareParams = {
  key: string;
  ttl_secs: number;
  download_filename?: string;
};
export type ShareLink = { url: string; expires_at_ms: number };

export type CredentialExportPayload = {
  encoded: string;
};

export type UsageDelta = {
  class_a: Record<string, number>;
  class_b: Record<string, number>;
  ingress_bytes: number;
  egress_bytes: number;
  added_storage_bytes: number;
  deleted_storage_bytes: number;
};
export type DailyLedger = {
  date: string;
  class_a: Record<string, number>;
  class_b: Record<string, number>;
  ingress_bytes: number;
  egress_bytes: number;
  storage_bytes: number;
  peak_storage_bytes: number;
  deleted_storage_bytes: number;
  rev: number;
  updated_at: string;
};

// New command: generate thumbnail and upload to R2
export type GenerateThumbnailAndUploadArgs = {
  key: string;
  sourcePath: string; // mapped to Rust source_path
};
