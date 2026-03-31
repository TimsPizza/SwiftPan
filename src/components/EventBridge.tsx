import type { DownloadEvent, UploadEvent } from "@/lib/api/bridge";
import {
  api,
  onDownloadEvent,
  onLogEvent,
  onUploadEvent,
} from "@/lib/api/tauriBridge";
import { formatBytes } from "@/lib/utils";
import { queryClient } from "@/main";
import { useAppStore } from "@/store/app-store";
import { useLogStore } from "@/store/log-store";
import { useTransferStore } from "@/store/transfer-store";
import {
  createChannel,
  Importance,
  isPermissionGranted,
  onAction,
  requestPermission,
  sendNotification,
  Visibility,
} from "@tauri-apps/plugin-notification";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect } from "react";
import { toast } from "sonner";

type TransferKind = "upload" | "download";

const isAndroidDevice =
  typeof navigator !== "undefined" &&
  /Android/i.test(String(navigator.userAgent || ""));
const isTauriApp =
  typeof window !== "undefined" &&
  Boolean(
    (window as unknown as { __TAURI_IPC__?: unknown }).__TAURI_IPC__ ??
      (window as unknown as { __TAURI_INTERNALS__?: unknown })
        .__TAURI_INTERNALS__,
  );

type NotificationPermissionState = "unknown" | "granted" | "denied";

let notificationPermissionState: NotificationPermissionState = "unknown";
let notificationInitPromise: Promise<boolean> | null = null;

function notificationKeyToId(key: string): number {
  // Basic 32-bit FNV-1a hash to turn stable string keys into signed ints.
  let hash = 2166136261;
  for (let i = 0; i < key.length; i += 1) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  const unsigned = hash >>> 0;
  return unsigned > 0x7fffffff ? unsigned - 0x100000000 : unsigned;
}

const TRANSFER_NOTIFICATION_PREFIX = "swiftpan.transfer.";
const PROGRESS_NOTIFICATION_KEY = "swiftpan.transfer.progress";
const PROGRESS_NOTIFICATION_ID = notificationKeyToId(PROGRESS_NOTIFICATION_KEY);
const PROGRESS_NOTIFICATION_CHANNEL_ID = "swiftpan.progress";
const DOWNLOAD_COMPLETE_EXTRA_KIND = "download_complete";
const DEFAULT_NOTIFICATION_ACTION = "tap";

type AggregateSnapshot = {
  active: number;
  percent: number;
  doneBytes: number;
  totalBytes: number;
  hasUnknown: boolean;
};

let lastProgressSnapshot: AggregateSnapshot | null = null;
let progressUpdateTimer: number | undefined;
let progressChannelPrepared = false;

function hasNotificationSupport() {
  return isAndroidDevice && isTauriApp;
}

async function ensureProgressChannel() {
  if (!hasNotificationSupport() || progressChannelPrepared) return;
  progressChannelPrepared = true;
  try {
    await createChannel({
      id: PROGRESS_NOTIFICATION_CHANNEL_ID,
      name: "Transfer progress",
      description: "SwiftPan transfer updates",
      vibration: false,
      lights: false,
      importance: Importance.Low,
      visibility: Visibility.Private,
    });
  } catch (err) {
    console.warn("progress notification channel setup failed", err);
  }
}

async function ensureNotificationReady(): Promise<boolean> {
  if (!hasNotificationSupport()) return false;
  if (notificationPermissionState === "granted") return true;
  if (notificationPermissionState === "denied") return false;
  if (!notificationInitPromise) {
    notificationInitPromise = (async () => {
      try {
        if (await isPermissionGranted()) {
          notificationPermissionState = "granted";
          return true;
        }
        const permission = await requestPermission();
        if (permission === "granted") {
          notificationPermissionState = "granted";
          return true;
        }
        notificationPermissionState = "denied";
        return false;
      } catch (err) {
        console.warn("notification permission lookup failed", err);
        notificationPermissionState = "denied";
        return false;
      }
    })();
  }
  if (!notificationInitPromise) return false;
  const ok = await notificationInitPromise;
  notificationPermissionState = ok ? "granted" : "denied";
  if (!ok && notificationPermissionState !== "granted") {
    notificationInitPromise = null;
  }
  return ok;
}

function computeAggregateSnapshot(): AggregateSnapshot | null {
  const items = Object.values(useTransferStore.getState().items);
  const running = items.filter((item) => item.state === "running");
  if (running.length === 0) return null;

  let totalBytes = 0;
  let doneBytes = 0;
  let unknown = 0;

  for (const item of running) {
    const total = item.bytesTotal ?? 0;
    if (total > 0) {
      totalBytes += total;
      const done = Math.min(item.bytesDone, total);
      if (Number.isFinite(done)) {
        doneBytes += done;
      }
    } else {
      unknown += 1;
    }
  }

  let percent = 0;
  if (totalBytes > 0) {
    percent = Math.floor((doneBytes / totalBytes) * 100);
  }
  percent = Math.max(0, Math.min(100, percent));

  return {
    active: running.length,
    percent,
    doneBytes,
    totalBytes,
    hasUnknown: unknown > 0,
  };
}

async function emitAggregateProgress(force = false): Promise<void> {
  if (!hasNotificationSupport()) return;
  if (!(await ensureNotificationReady())) return;

  const summary = computeAggregateSnapshot();
  if (!summary) {
    if (!lastProgressSnapshot) return;
    lastProgressSnapshot = null;
    try {
      await sendNotification({
        id: PROGRESS_NOTIFICATION_ID,
        title: "Transfers complete",
        body: "All transfers have finished.",
      });
    } catch (err) {
      console.warn("aggregate progress completion notification failed", err);
    }
    return;
  }

  if (
    !force &&
    lastProgressSnapshot &&
    lastProgressSnapshot.active === summary.active &&
    lastProgressSnapshot.percent === summary.percent &&
    lastProgressSnapshot.totalBytes === summary.totalBytes &&
    lastProgressSnapshot.doneBytes === summary.doneBytes &&
    lastProgressSnapshot.hasUnknown === summary.hasUnknown
  ) {
    return;
  }

  lastProgressSnapshot = summary;
  await ensureProgressChannel();

  const pieces = [`Active: ${summary.active}`, `Progress: ${summary.percent}%`];
  if (summary.totalBytes > 0) {
    pieces.push(
      `Transferred ${formatBytes(summary.doneBytes)} / ${formatBytes(summary.totalBytes)}`,
    );
  } else if (summary.hasUnknown) {
    pieces.push("Waiting for total size");
  }

  try {
    await sendNotification({
      id: PROGRESS_NOTIFICATION_ID,
      title: "Transfers running",
      body: pieces.join(" · "),
      channelId: PROGRESS_NOTIFICATION_CHANNEL_ID,
      ongoing: true,
      autoCancel: false,
    });
  } catch (err) {
    console.warn("aggregate progress notification failed", err);
  }
}

function triggerAggregateUpdate(force = false) {
  if (!hasNotificationSupport()) return;
  if (force) {
    void emitAggregateProgress(true);
    return;
  }
  if (typeof window === "undefined") return;
  if (progressUpdateTimer !== undefined) return;
  progressUpdateTimer = window.setTimeout(() => {
    progressUpdateTimer = undefined;
    void emitAggregateProgress(false);
  }, 900);
}

type TransferNotificationExtras = {
  extra?: Record<string, unknown>;
  actionTypeId?: string;
};

async function notifyTransferEvent(
  kind: TransferKind,
  id: string,
  title: string,
  body?: string,
  options?: TransferNotificationExtras,
): Promise<void> {
  if (!hasNotificationSupport()) return;
  if (!(await ensureNotificationReady())) return;
  const content =
    body && body.trim().length > 0 ? body : `${kind} ${String(id)}`;
  const numericId = notificationKeyToId(`${TRANSFER_NOTIFICATION_PREFIX}${id}`);
  try {
    await sendNotification({
      id: numericId,
      title,
      body: content,
      actionTypeId: options?.actionTypeId,
      extra: options?.extra,
    });
  } catch (err) {
    console.warn(`send notification failed for ${kind}`, err);
  }
}

function extractNotificationExtra(
  what: any,
): Record<string, unknown> | undefined {
  if (!what || typeof what !== "object") return undefined;
  const raw = (what.extra ?? what.data) as unknown;
  if (!raw) return undefined;
  if (typeof raw === "string") {
    try {
      return JSON.parse(raw);
    } catch {
      return undefined;
    }
  }
  if (typeof raw === "object") {
    return raw as Record<string, unknown>;
  }
  return undefined;
}

async function openAndroidDownloadDirectoryFromNotification() {
  if (!isAndroidDevice) return;
  try {
    const store = useAppStore.getState();
    const setter = store.setAndroidTreeUri;
    let treeUri = store.androidTreeUri;
    if (!treeUri) {
      try {
        const persisted = await api.android_get_persisted_download_dir();
        if (persisted) {
          treeUri = persisted;
          setter?.(persisted);
        }
      } catch (err) {
        console.warn("failed to resolve persisted android download dir", err);
      }
    }
    if (!treeUri) {
      toast.error("Download directory not available");
      return;
    }
    await openUrl(treeUri);
  } catch (err) {
    console.warn("open download directory via notification failed", err);
    toast.error("Failed to open download directory");
  }
}

async function handleNotificationActionEvent(payload: any) {
  if (!payload || typeof payload !== "object") return;
  const actionId =
    typeof payload.actionId === "string" ? payload.actionId : undefined;
  if (actionId !== DEFAULT_NOTIFICATION_ACTION) return;
  const notification = payload.notification;
  const extra = extractNotificationExtra(notification);
  const spKind =
    typeof extra?.spKind === "string"
      ? extra.spKind
      : typeof extra?.sp_kind === "string"
        ? extra.sp_kind
        : undefined;
  if (spKind === DOWNLOAD_COMPLETE_EXTRA_KIND) {
    await openAndroidDownloadDirectoryFromNotification();
  }
}

async function refreshUploadStatus(id: string) {
  try {
    const status = await api.upload_status(id);
    if (!status) {
      useTransferStore.getState().remove(id);
      triggerAggregateUpdate(true);
      return;
    }
    const store = useTransferStore.getState();
    // Ensure basic item exists
    store.update(id, {
      id,
      type: "upload",
      key: status.key,
      phase: status.phase,
      bytesTotal: status.bytes_total,
      bytesDone: status.bytes_done,
      rateBps: status.rate_bps ?? 0,
      state: status.lifecycle_state as any,
      error: status.last_error
        ? String(
            (status.last_error as any).message ||
              (status.last_error as any).kind ||
              "upload failed",
          )
        : undefined,
    });
    triggerAggregateUpdate();
  } catch (err: any) {
    console.warn("upload_status failed", id, err);
  }
}

async function refreshDownloadStatus(id: string) {
  try {
    const status = await api.download_status(id);
    if (!status) {
      useTransferStore.getState().remove(id);
      triggerAggregateUpdate(true);
      return;
    }
    const store = useTransferStore.getState();
    store.update(id, {
      id,
      type: "download",
      key: status.key,
      phase: status.phase,
      bytesTotal: status.bytes_total,
      bytesDone: status.bytes_done,
      rateBps: status.rate_bps ?? 0,
      tempPath: status.temp_path,
      state: status.lifecycle_state as any,
      error: status.last_error
        ? String(
            (status.last_error as any).message ||
              (status.last_error as any).kind ||
              "download failed",
          )
        : undefined,
    });
    triggerAggregateUpdate();
  } catch (err: any) {
    console.warn("download_status failed", id, err);
  }
}

function handleUploadEvent(ev: UploadEvent) {
  const id = ev.transfer_id;
  const keyOrName = () => {
    try {
      const item = useTransferStore.getState().items[id];
      return item?.key ? String(item.key) : undefined;
    } catch {
      return undefined;
    }
  };
  switch (ev.type) {
    case "Started": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      {
        const k = keyOrName();
        toast.info(k ? `Upload started: ${k}` : "Upload started");
        void notifyTransferEvent("upload", id, "Upload started", k ?? id);
      }
      break;
    }
    case "Resumed": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "Paused": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "Cancelling": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "PartDone": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "Completed": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      {
        const k = keyOrName();
        toast.success(k ? `Upload completed: ${k}` : "Upload completed");
        void notifyTransferEvent("upload", id, "Upload completed", k ?? id);
      }
      triggerAggregateUpdate(true);
      queryClient.invalidateQueries({
        queryKey: ["list_all_objects"],
        refetchType: "active",
      });
      queryClient.refetchQueries({
        queryKey: ["list_all_objects"],
        type: "all",
      });
      console.log("query invalidated");
      break;
    }
    case "Failed": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      {
        const k = keyOrName();
        const msg = ev.error?.message ? `: ${ev.error.message}` : "";
        toast.error(k ? `Upload failed (${k})${msg}` : `Upload failed${msg}`);
        const detail = k ?? id;
        const notifMsg = ev.error?.message ? ` · ${ev.error.message}` : "";
        void notifyTransferEvent(
          "upload",
          id,
          "Upload failed",
          `${detail}${notifMsg}`,
        );
      }
      break;
    }
    case "Cancelled": {
      setTimeout(() => void refreshUploadStatus(id), 0);
      {
        const k = keyOrName();
        toast.info(k ? `Upload cancelled: ${k}` : "Upload cancelled");
        void notifyTransferEvent("upload", id, "Upload cancelled", k ?? id);
      }
      break;
    }
  }
}

async function handleDownloadEvent(ev: DownloadEvent) {
  const id = ev.transfer_id;
  console.log("handleDownloadEvent", ev);
  const keyOrName = () => {
    try {
      const item = useTransferStore.getState().items[id];
      return item?.key ? String(item.key) : undefined;
    } catch {
      return undefined;
    }
  };
  switch (ev.type) {
    case "Started": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      {
        const k = keyOrName();
        toast.info(k ? `Download started: ${k}` : "Download started");
        void notifyTransferEvent("download", id, "Download started", k ?? id);
      }
      break;
    }
    case "Resumed": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "Paused": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "Cancelling": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "ChunkDone": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "Completed": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      const k = keyOrName();
      toast.success(k ? `Download completed: ${k}` : "Download completed");
      {
        void notifyTransferEvent(
          "download",
          id,
          "Download completed",
          k ?? id,
          {
            extra: {
              spKind: DOWNLOAD_COMPLETE_EXTRA_KIND,
              key: k ?? id,
            },
          },
        );
      }
      break;
    }
    case "Failed": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      {
        const k = keyOrName();
        const msg = ev.error?.message ? `: ${ev.error.message}` : "";
        toast.error(
          k ? `Download failed (${k})${msg}` : `Download failed${msg}`,
        );
        const detail = k ?? id;
        const notifMsg = ev.error?.message ? ` · ${ev.error.message}` : "";
        void notifyTransferEvent(
          "download",
          id,
          "Download failed",
          `${detail}${notifMsg}`,
        );
      }
      break;
    }
    case "Cancelled": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      {
        const k = keyOrName();
        toast.info(k ? `Download cancelled: ${k}` : "Download cancelled");
        void notifyTransferEvent("download", id, "Download cancelled", k ?? id);
      }
      break;
    }
    case "SourceChanged": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      {
        const k = keyOrName();
        void notifyTransferEvent(
          "download",
          id,
          "Download failed",
          `${k ?? id} · source changed`,
        );
      }
      break;
    }
  }
}

// Idempotent, recoverable singleton initializer stored on window
async function ensureBridgeOnce() {
  const w = window as unknown as {
    __SP_EVENT_BRIDGE__?: {
      initialized: boolean;
      unlistenUpload?: () => void;
      unlistenDownload?: () => void;
      unlistenNotificationAction?: () => void;
      poller?: number;
    };
  };
  if (w.__SP_EVENT_BRIDGE__?.initialized) return;
  const ctrl: NonNullable<typeof w.__SP_EVENT_BRIDGE__> = {
    initialized: true,
    unlistenUpload: undefined,
    unlistenDownload: undefined,
    unlistenNotificationAction: undefined,
    poller: undefined,
  };
  w.__SP_EVENT_BRIDGE__ = ctrl;

  // Subscribe once
  try {
    const unLog = await onLogEvent((payload) => {
      console.log("log evt: ", payload);
      const p = payload as any;
      const e = p && typeof p === "object" ? p : null;
      if (!e) return;
      useLogStore.getState().append({
        ts: String(e.ts || ""),
        level: String(e.level || ""),
        target: String(e.target || ""),
        message: String(e.message || e.line || ""),
        line: String(e.line || ""),
      });
    });
    // @ts-ignore add dynamic property to store unlisten
    ctrl.unlistenLog = unLog;
  } catch {}

  try {
    const unUpload = await onUploadEvent((payload) => {
      console.log("event upload", payload);
      // Defensive parsing
      const ev = payload as UploadEvent;
      if (!ev || typeof ev !== "object" || !("type" in ev)) return;
      // Defer to avoid nested React effects
      setTimeout(() => handleUploadEvent(ev), 0);
    });
    ctrl.unlistenUpload = unUpload;
  } catch {
    // ignore
  }
  try {
    const unDownload = await onDownloadEvent((payload) => {
      console.log("event download", payload);
      const ev = payload as DownloadEvent;
      if (!ev || typeof ev !== "object" || !("type" in ev)) return;
      setTimeout(() => handleDownloadEvent(ev), 0);
    });
    ctrl.unlistenDownload = unDownload;
  } catch {
    // ignore
  }

  if (hasNotificationSupport()) {
    try {
      const actionListener = await onAction((payload) => {
        void handleNotificationActionEvent(payload);
      });
      ctrl.unlistenNotificationAction = () => {
        void actionListener.unregister();
      };
    } catch (err) {
      console.warn("notification action listener setup failed", err);
    }
  }

  try {
    const active = await api.transfer_list_active();
    for (const item of active || []) {
      useTransferStore.getState().update(item.transfer_id, {
        id: item.transfer_id,
        type: item.kind,
        key: item.key,
        state: item.lifecycle_state as any,
        phase: item.phase,
        bytesTotal: item.bytes_total,
        bytesDone: item.bytes_done,
        rateBps: item.rate_bps ?? 0,
        error: item.last_error?.message,
        destPath: item.dest_path,
        tempPath: item.temp_path,
      });
      setTimeout(
        () =>
          void (item.kind === "upload"
            ? refreshUploadStatus(item.transfer_id)
            : refreshDownloadStatus(item.transfer_id)),
        0,
      );
    }
    triggerAggregateUpdate(true);
  } catch (err) {
    console.warn("transfer_list_active failed", err);
  }

  // Seed logger status and initial tail once
  try {
    const st = await api.log_get_status();
    if (st) useLogStore.getState().setStatus(st as any);
    const tail = await api.log_tail(400);
    if (tail) useLogStore.getState().setAll(String(tail).split("\n"));
  } catch {}

  // Load app settings to seed store
  try {
    const s = await api.settings_get();
    if (s && typeof s === "object") {
      useAppStore.getState().setSettings({
        logLevel: String((s as any).logLevel ?? "info"),
        maxConcurrency: Number((s as any).maxConcurrency ?? 2),
        defaultDownloadDir:
          (s as any).defaultDownloadDir != null
            ? String((s as any).defaultDownloadDir)
            : null,
        uploadThumbnail: Boolean((s as any).uploadThumbnail ?? false),
        androidTreeUri:
          (s as any).androidTreeUri != null
            ? String((s as any).androidTreeUri)
            : null,
      });
    }
  } catch {}

  // Poll active transfers periodically to stay in sync (idempotent)
  ctrl.poller = window.setInterval(async () => {
    const active = Object.values(useTransferStore.getState().items).filter(
      (item) =>
        item.state !== "completed" &&
        item.state !== "failed" &&
        item.state !== "cancelled",
    );
    if (active.length === 0) return;
    // Limit per tick to avoid bursts
    const batch = active.slice(0, 6);
    await Promise.all(
      batch.map((item) =>
        item.type === "upload"
          ? refreshUploadStatus(item.id)
          : refreshDownloadStatus(item.id),
      ),
    );
  }, 1500);
}

export default function EventBridge() {
  useEffect(() => {
    void ensureBridgeOnce();
  }, []);
  return null;
}
