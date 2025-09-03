import { invoke } from "@tauri-apps/api/core";
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

export async function vault_status() {
  return invoke("vault_status");
}

export async function vault_set_manual(bundle: unknown, master_password: string) {
  return invoke("vault_set_manual", { bundle, masterPassword: master_password });
}

export async function vault_unlock(master_password: string, hold_ms: number) {
  return invoke("vault_unlock", { masterPassword: master_password, holdMs: hold_ms });
}

export async function vault_lock() {
  return invoke("vault_lock");
}

export async function r2_sanity_check() {
  return invoke("r2_sanity_check");
}

export async function list_objects(prefix = "", token?: string, max_keys = 1000): Promise<ListPage> {
  return invoke("list_objects", { prefix, token, maxKeys: max_keys }) as Promise<ListPage>;
}

export async function delete_object(key: string) {
  return invoke("delete_object", { key });
}

export async function share_generate(params: { key: string; ttl_secs: number; download_filename?: string }) {
  return invoke("share_generate", { params }) as Promise<ShareLink>;
}

export async function download_now(key: string, dest_path: string) {
  return invoke("download_now", { key, destPath: dest_path });
}

export async function usage_merge_day(date: string) {
  return invoke("usage_merge_day", { date }) as Promise<DailyLedger>;
}

export async function usage_list_month(prefix: string) {
  return invoke("usage_list_month", { prefix }) as Promise<DailyLedger[]>;
}

export const ANALYTICS_PREFIX = "analytics/daily/";

