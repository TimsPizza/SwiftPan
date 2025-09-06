import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ResultAsync } from "neverthrow";
import {
  useMutation,
  UseMutationOptions,
  useQuery,
  UseQueryOptions,
} from "react-query";
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
  settings_get: () =>
    resultInvoke<{
      logLevel: string;
      maxConcurrency: number;
      defaultDownloadDir?: string | null;
      uploadThumbnail: boolean;
    }>("settings_get"),
  settings_set: (settings: {
    logLevel: string;
    maxConcurrency: number;
    defaultDownloadDir?: string | null;
    uploadThumbnail: boolean;
  }) =>
    resultInvoke<void>("settings_set", {
      settings,
    }),
  backend_set_credentials: (bundle: unknown) =>
    resultInvoke<void>("backend_set_credentials", {
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
    resultInvoke<void>("backend_patch_credentials", {
      patch,
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
  vault_set_manual: (bundle: unknown) =>
    resultInvoke<void>("vault_set_manual", {
      bundle,
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
  share_list: () =>
    resultInvoke<
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
  usage_month_cost: (prefix: string) =>
    resultInvoke<{
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
  log_tail: (lines?: number) => resultInvoke<string>("log_tail", { lines }),
  log_clear: () => resultInvoke<void>("log_clear"),
  log_set_level: (level: "trace" | "debug" | "info" | "warn" | "error") =>
    resultInvoke<void>("log_set_level", { level }),
  log_get_status: () =>
    resultInvoke<{
      level: string;
      cache_lines: number;
      file_path: string;
      file_size_bytes: number;
    }>("log_get_status"),
};

// React Query wrappers (minimal, non-breaking)
export const queries = {
  useSettings: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["app_settings"],
      queryFn: async () => (await nv.settings_get()).unwrapOr(null),
      staleTime: 30_000,
      ...opts,
    }),
  useListAllObjects: (
    maxTotal = 10000,
    opts?: UseQueryOptions<any, unknown, any, any>,
  ) =>
    useQuery({
      queryKey: ["list_all_objects", maxTotal],
      queryFn: async () => (await nv.list_all_objects(maxTotal)).unwrapOr([]),
      staleTime: 300_000,
      cacheTime: 600_000,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
      ...opts,
    }),
  useBackendStatus: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["backend_status"],
      queryFn: async () => (await nv.backend_status()).unwrapOr({}),
      staleTime: 10_000,
      ...opts,
    }),
  useBackendCredentialsRedacted: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["backend_credentials_redacted"],
      queryFn: async () =>
        (await nv.backend_credentials_redacted()).unwrapOr(null),
      ...opts,
    }),
  useUsageListMonth: (prefix: string, opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["usage_list_month", prefix],
      queryFn: async () => (await nv.usage_list_month(prefix)).unwrapOr([]),
      ...opts,
    }),
  useUsageMonthCost: (prefix: string, opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["usage_month_cost", prefix],
      queryFn: async () => (await nv.usage_month_cost(prefix)).unwrapOr(null),
      ...opts,
    }),
  useLogStatus: (opts?: UseQueryOptions<any>) =>
    useQuery({
      queryKey: ["log_status"],
      queryFn: async () => (await nv.log_get_status()).unwrapOr(null),
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
          await await nv.backend_set_credentials(bundle).unwrapOr(undefined);
        } else {
          await await nv.backend_patch_credentials(bundle).unwrapOr(undefined);
        }
      },
      ...opts,
    }),
  useR2Sanity: (opts?: UseMutationOptions<void, unknown, void>) =>
    useMutation({
      mutationFn: async () => {
        const r = await nv.r2_sanity_check();
        if (r.isErr()) {
          throw r.error;
        }
      },
      ...opts,
    }),
  useUsageMergeDay: (opts?: UseMutationOptions<void, unknown, string>) =>
    useMutation({
      mutationFn: async (date: string) => {
        await (await nv.usage_merge_day(date)).unwrapOr(undefined);
      },
      ...opts,
    }),
};
