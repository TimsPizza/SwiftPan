import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ResultAsync } from "neverthrow";
import type { DailyLedger, ShareLink } from "./bridge";

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
  list_all_objects: (max_total = 10000) =>
    resultInvoke<
      {
        key: string;
        size?: number;
        last_modified_ms?: number;
        etag?: string;
        is_prefix: boolean;
        protected: boolean;
      }[]
    >("list_all_objects", { maxTotal: max_total }),
  delete_object: (key: string) => resultInvoke<void>("delete_object", { key }),
  share_generate: (params: {
    key: string;
    ttl_secs: number;
    download_filename?: string;
  }) => resultInvoke<ShareLink>("share_generate", { params }),
  download_now: (key: string, dest_path: string) =>
    resultInvoke<void>("download_now", { key, destPath: dest_path }),
  usage_merge_day: (date: string) =>
    resultInvoke<DailyLedger>("usage_merge_day", { date }),
  usage_list_month: (prefix: string) =>
    resultInvoke<DailyLedger[]>("usage_list_month", { prefix }),
};
