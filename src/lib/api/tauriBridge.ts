import {
  useMutation,
  UseMutationOptions,
  useQuery,
  UseQueryOptions,
} from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CredentialExportPayload,
  DailyLedger,
  ShareLink,
  UploadStatus,
} from "./bridge";

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
export function onLogEvent(cb: (event: unknown) => void) {
  return listen("sp://log_event", (e) => cb(e.payload));
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

// Thin helper to run every Tauri call through the debug logger.
function invokeBridge<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
) {
  return debugInvoke<T>(cmd, args);
}

export const api = {
  settings_get: () =>
    invokeBridge<{
      logLevel: string;
      maxConcurrency: number;
      defaultDownloadDir?: string | null;
      uploadThumbnail: boolean;
      androidTreeUri?: string | null;
    }>("settings_get"),
  settings_set: (settings: {
    logLevel: string;
    maxConcurrency: number;
    defaultDownloadDir?: string | null;
    uploadThumbnail: boolean;
    androidTreeUri?: string | null;
  }) =>
    invokeBridge<void>("settings_set", {
      settings,
    }),
  backend_set_credentials: (bundle: unknown) =>
    invokeBridge<void>("backend_set_credentials", {
      bundle,
    }),
  backend_patch_credentials: (
    patch: Partial<{
      endpoint: string;
      access_key_id: string;
      secret_access_key: string;
      bucket: string;
      region?: string;
    }>,
  ) =>
    invokeBridge<void>("backend_patch_credentials", {
      patch,
    }),
  backend_status: () => invokeBridge("backend_status"),
  backend_credentials_redacted: () =>
    invokeBridge<{
      endpoint: string;
      access_key_id: string;
      secret_access_key: string;
      bucket: string;
      region?: string;
    }>("backend_credentials_redacted"),
  backend_export_credentials_package: () =>
    invokeBridge<CredentialExportPayload>("backend_export_credentials_package"),
  backend_import_credentials_package: (encoded: string) =>
    invokeBridge<void>("backend_import_credentials_package", {
      encoded,
    }),
  vault_set_manual: (bundle: unknown) =>
    invokeBridge<void>("vault_set_manual", {
      bundle,
    }),
  vault_status: () => invokeBridge("vault_status"),
  r2_sanity_check: () => invokeBridge("r2_sanity_check"),
  // deprecated
  list_objects: (prefix = "", token?: string, max = 1000) =>
    invokeBridge<ListPage>("list_objects", {
      prefix,
      token,
      maxKeys: max,
    }),
  list_all_objects: (maxTotal = 10000) =>
    invokeBridge<
      {
        key: string;
        size?: number;
        last_modified_ms?: number;
        etag?: string;
        is_prefix: boolean;
        protected: boolean;
      }[]
    >("list_all_objects", { maxTotal }),
  delete_object: (key: string) =>
    invokeBridge<string>("delete_object", { key }),
  share_generate: (params: {
    key: string;
    ttl_secs: number;
    download_filename?: string;
  }) => invokeBridge<ShareLink>("share_generate", { params }),
  share_list: () =>
    invokeBridge<
      {
        key: string;
        url: string;
        created_at_ms: number;
        expires_at_ms: number;
        ttl_secs: number;
        download_filename?: string;
      }[]
    >("share_list"),
  // Upload controls
  upload_new: (params: {
    key: string;
    source_path: string;
    part_size: number;
    content_type?: string;
    content_disposition?: string;
  }) => invokeBridge<string>("upload_new", { params }),
  upload_new_stream: (params: {
    key: string;
    bytes_total: number;
    part_size: number;
    content_type?: string;
    content_disposition?: string;
  }) => invokeBridge<string>("upload_new_stream", { params }),
  upload_stream_write: (transferId: string, chunk: Uint8Array | number[]) =>
    invokeBridge<void>("upload_stream_write", {
      transferId,
      chunk: Array.from(chunk as any),
    }),
  upload_stream_finish: (transferId: string) =>
    invokeBridge<void>("upload_stream_finish", { transferId }),
  upload_ctrl: (transferId: string, action: "pause" | "resume" | "cancel") =>
    invokeBridge<void>("upload_ctrl", { transferId, action }),
  upload_status: (transferId: string) =>
    invokeBridge<UploadStatus>("upload_status", { transferId }),
  download_new: (params: {
    key: string;
    dest_path: string;
    chunk_size: number;
    expected_etag?: string;
  }) => invokeBridge<string>("download_new", { params }),
  download_ctrl: (transferId: string, action: "pause" | "resume" | "cancel") =>
    invokeBridge<void>("download_ctrl", { transferId, action }),
  download_status: (transferId: string) =>
    invokeBridge<{
      transfer_id: string;
      key: string;
      bytes_total?: number;
      bytes_done: number;
      rate_bps: number;
      expected_etag?: string;
      observed_etag?: string;
      last_error?: unknown;
    }>("download_status", { transferId }),
  download_sandbox_dir: () => invokeBridge<string>("download_sandbox_dir"),
  usage_merge_day: (date: string) =>
    invokeBridge<DailyLedger>("usage_merge_day", { date }),
  usage_list_month: (prefix: string) =>
    invokeBridge<DailyLedger[]>("usage_list_month", { prefix }),
  usage_month_cost: (prefix: string) =>
    invokeBridge<{
      month: string;
      storage: {
        sum_peak_gb: number;
        avg_gb_month_ceil: number;
        free_gb_month: number;
        billable_gb_month: number;
        cost_usd: number;
      };
      class_a: {
        total_ops: number;
        free_ops: number;
        billable_millions: number;
        unit_price: number;
        cost_usd: number;
      };
      class_b: {
        total_ops: number;
        free_ops: number;
        billable_millions: number;
        unit_price: number;
        cost_usd: number;
      };
      total_cost_usd: number;
    }>("usage_month_cost", { prefix }),
  // Logs
  log_tail: (lines?: number) => invokeBridge<string>("log_tail", { lines }),
  log_clear: () => invokeBridge<void>("log_clear"),
  log_set_level: (level: "trace" | "debug" | "info" | "warn" | "error") =>
    invokeBridge<void>("log_set_level", { level }),
  log_get_status: () =>
    invokeBridge<{
      level: string;
      cache_lines: number;
      file_path: string;
      file_size_bytes: number;
    }>("log_get_status"),
  ui_status_bar_height: () => invokeBridge<number>("ui_status_bar_height"),
  // Android SAF (Storage Access Framework) APIs
  android_pick_download_dir: () =>
    invokeBridge<string>("android_pick_download_dir"),
  android_get_persisted_download_dir: () =>
    invokeBridge<string | null>("android_get_persisted_download_dir"),
  android_fs_copy: (params: {
    direction: "sandbox_to_tree" | "tree_to_sandbox" | "uri_to_sandbox";
    local_path: string;
    tree_uri?: string;
    relative_path?: string;
    mime?: string;
    uri?: string;
  }) => invokeBridge<void>("android_fs_copy", { params }),
  // Android uploads via SAF (native; avoids JS streaming)
  android_pick_upload_files: () =>
    invokeBridge<Array<{ uri: string; name: string; size?: number }>>(
      "android_pick_upload_files",
    ),
  android_upload_from_uri: (params: {
    key: string;
    uri: string;
    part_size: number;
  }) => invokeBridge<string>("android_upload_from_uri", { params }),
};

export async function applyStatusBarInsetFromNative() {
  try {
    const nativeValue = await api.ui_status_bar_height();
    console.log(
      "[tauriBridge] fetched status bar height (native px):",
      nativeValue,
    );
    // Native returns physical pixels on both Android (px) and iOS (points * scale).
    // CSS expects logical pixels. Convert by dividing devicePixelRatio.
    const nativePx = nativeValue ?? 0;
    const dpr = Math.max(1, (globalThis as any).devicePixelRatio || 1);
    const cssPxFromNative = Math.max(0, Math.round(nativePx / dpr));

    // Also consider current visual viewport offset as a sanity check.
    const vv: any = (globalThis as any).visualViewport;
    const vvTop =
      vv && typeof vv.offsetTop === "number"
        ? Math.max(0, Math.round(vv.offsetTop))
        : 0;

    // Keep the max of: previous value, vvTop, and native-derived value.
    const prevStr = getComputedStyle(document.documentElement)
      .getPropertyValue("--fallback-top")
      .trim();
    const prev = Number.parseInt(prevStr, 10) || 0;
    const resolvedTop = Math.max(prev, vvTop, cssPxFromNative);

    const v = `${resolvedTop}px`;
    (globalThis as any).__SP_STATUS_BAR__ = resolvedTop;
    document.documentElement.style.setProperty("--fallback-top", v);
    try {
      localStorage.setItem("sp:fallback-top", v);
    } catch {}
    return resolvedTop;
  } catch {
    return 0;
  }
}

// React Query wrappers (minimal, non-breaking)
export const queries = {
  useSettings: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["app_settings"] as const,
      queryFn: async () => {
        const settings = await api.settings_get();
        return settings ?? null;
      },
      staleTime: 30_000,
      ...opts,
    }),
  useListAllObjects: (
    maxTotal = 10000,
    opts?: Partial<UseQueryOptions<any, unknown, any, any>>,
  ) =>
    useQuery({
      queryKey: ["list_all_objects"] as const,
      queryFn: async () => {
        const items = await api.list_all_objects(maxTotal);
        return items ?? [];
      },
      // staleTime: 30_000,
      // gcTime: 60_000,
      // refetchOnWindowFocus: false,
      // refetchOnReconnect: false,
      ...opts,
    }),
  useBackendStatus: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["backend_status"] as const,
      queryFn: async () => {
        const status = await api.backend_status();
        return status ?? {};
      },
      staleTime: 10_000,
      ...opts,
    }),
  useBackendCredentialsRedacted: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["backend_credentials_redacted"] as const,
      queryFn: async () => {
        const creds = await api.backend_credentials_redacted();
        return creds ?? null;
      },
      ...opts,
    }),
  useUsageListMonth: (prefix: string, opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["usage_list_month", prefix] as const,
      queryFn: async () => {
        const usage = await api.usage_list_month(prefix);
        return usage ?? [];
      },
      ...opts,
    }),
  useUsageMonthCost: (prefix: string, opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["usage_month_cost", prefix] as const,
      queryFn: async () => {
        const cost = await api.usage_month_cost(prefix);
        return cost ?? null;
      },
      ...opts,
    }),
  useLogStatus: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["log_status"] as const,
      queryFn: async () => {
        const status = await api.log_get_status();
        return status ?? null;
      },
      refetchInterval: 10_000,
      ...opts,
    }),
};

export const mutations = {
  useSaveCredentials: (opts?: UseMutationOptions<void, unknown, any>) =>
    useMutation({
      mutationFn: async (bundle: any) => {
        // For compatibility, accept either full bundle { r2: {...} } or direct patch shape
        if (bundle && bundle.r2) {
          await api.backend_set_credentials(bundle);
        } else {
          await api.backend_patch_credentials(bundle);
        }
      },
      ...opts,
    }),
  useR2Sanity: (opts?: UseMutationOptions<void, unknown, void>) =>
    useMutation({
      mutationFn: async () => {
        await api.r2_sanity_check();
      },
      ...opts,
    }),
  useUsageMergeDay: (opts?: UseMutationOptions<void, unknown, string>) =>
    useMutation({
      mutationFn: async (date: string) => {
        await api.usage_merge_day(date);
      },
      ...opts,
    }),
  useExportCredentialsPackage: (
    opts?: UseMutationOptions<CredentialExportPayload, unknown, void>,
  ) =>
    useMutation({
      mutationFn: async () => {
        return api.backend_export_credentials_package();
      },
      ...opts,
    }),
  useImportCredentialsPackage: (
    opts?: UseMutationOptions<void, unknown, { encoded: string }>,
  ) =>
    useMutation({
      mutationFn: async ({ encoded }) => {
        await api.backend_import_credentials_package(encoded);
      },
      ...opts,
    }),
};
