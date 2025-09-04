import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ResultAsync } from "neverthrow";
import type { DailyLedger, ShareLink, UploadStatus } from "./bridge";

export type ListPage = {
  prefix: string;
  items: Array<{
    key: string;
    size?: number;
    last_modified_ms?: number;
    etag?: string;
    is_prefix: boolean;
    protected: boolean;
  }>;
  next_token?: string;
};

export const ANALYTICS_PREFIX = "analytics/daily/";

// Event subscriptions
export function onUploadEvent(cb: (event: unknown) => void) {
  return listen("sp://upload_event", (e) => cb(e.payload));
}
export function onDownloadEvent(cb: (event: unknown) => void) {
  return listen("sp://download_event", (e) => cb(e.payload));
}
export function onBackgroundStats(cb: (event: unknown) => void) {
  return listen("sp://background_stats", (e) => cb(e.payload));
}

export async function bg_mock_start() {
  return debugInvoke("bg_mock_start");
}

async function debugInvoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const start = Date.now();
  // eslint-disable-next-line no-console
  console.log(`[invoke] ${cmd}`, args);
  try {
    const res = await invoke<T>(cmd, args as any);
    const ms = Date.now() - start;
    // eslint-disable-next-line no-console
    console.log(`[invoke:ok] ${cmd} (${ms}ms)`, res);
    return res;
  } catch (e) {
    const ms = Date.now() - start;
    // eslint-disable-next-line no-console
    console.error(`[invoke:err] ${cmd} (${ms}ms)`, e);
    throw e;
  }
}

// neverthrow ResultAsync wrappers (non-breaking; optional usage)
function resultInvoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
) {
  return ResultAsync.fromPromise<T, unknown>(
    debugInvoke<T>(cmd, args),
    (e) => e,
  );
}

export const nv = {
  backend_set_credentials: (bundle: unknown, masterPassword: string) =>
    resultInvoke<void>("backend_set_credentials", {
      bundle,
      masterPassword,
    }),
  backend_status: () => resultInvoke("backend_status"),
  backend_credentials_redacted: () =>
    resultInvoke<{
      endpoint: string;
      access_key_id: string;
      secret_access_key: string;
      bucket: string;
      region?: string;
    }>("backend_credentials_redacted"),
  vault_set_manual: (bundle: unknown, masterPassword: string) =>
    resultInvoke<void>("vault_set_manual", {
      bundle,
      masterPassword,
    }),
  vault_status: () => resultInvoke("vault_status"),
  r2_sanity_check: () => resultInvoke("r2_sanity_check"),
  list_objects: (prefix = "", token?: string, max = 1000) =>
    resultInvoke<ListPage>("list_objects", {
      prefix,
      token,
      maxKeys: max,
    }),
  list_all_objects: (maxTotal = 10000) =>
    resultInvoke<
      {
        key: string;
        size?: number;
        last_modified_ms?: number;
        etag?: string;
        is_prefix: boolean;
        protected: boolean;
      }[]
    >("list_all_objects", { maxTotal }),
  delete_object: (key: string) => resultInvoke<void>("delete_object", { key }),
  share_generate: (params: {
    key: string;
    ttl_secs: number;
    download_filename?: string;
  }) => resultInvoke<ShareLink>("share_generate", { params }),
  // Upload controls
  upload_new: (params: {
    key: string;
    source_path: string;
    part_size: number;
    content_type?: string;
    content_disposition?: string;
  }) => resultInvoke<string>("upload_new", { params }),
  upload_ctrl: (transferId: string, action: "pause" | "resume" | "cancel") =>
    resultInvoke<void>("upload_ctrl", { transferId, action }),
  upload_status: (transferId: string) =>
    resultInvoke<UploadStatus>("upload_status", { transferId }),
  download_now: (key: string, destPath: string) =>
    resultInvoke<void>("download_now", { key, destPath }),
  download_new: (params: {
    key: string;
    dest_path: string;
    chunk_size: number;
    expected_etag?: string;
  }) => resultInvoke<string>("download_new", { params }),
  download_ctrl: (transferId: string, action: "pause" | "resume" | "cancel") =>
    resultInvoke<void>("download_ctrl", { transferId, action }),
  download_status: (transferId: string) =>
    resultInvoke<{
      transfer_id: string;
      key: string;
      bytes_total?: number;
      bytes_done: number;
      rate_bps: number;
      expected_etag?: string;
      observed_etag?: string;
      last_error?: unknown;
    }>("download_status", { transferId }),
  usage_merge_day: (date: string) =>
    resultInvoke<DailyLedger>("usage_merge_day", { date }),
  usage_list_month: (prefix: string) =>
    resultInvoke<DailyLedger[]>("usage_list_month", { prefix }),
};
