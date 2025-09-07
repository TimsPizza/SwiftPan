import type {
  DownloadEvent,
  DownloadStatus,
  UploadEvent,
  UploadStatus,
} from "@/lib/api/bridge";
import {
  nv,
  onDownloadEvent,
  onLogEvent,
  onUploadEvent,
} from "@/lib/api/tauriBridge";
import { useAppStore } from "@/store/app-store";
import { useLogStore } from "@/store/log-store";
import { useTransferStore } from "@/store/transfer-store";
import { open, remove, stat } from "@tauri-apps/plugin-fs";
import { useEffect } from "react";

type TransferKind = "upload" | "download";

type ActiveRegistry = Record<string, TransferKind>;

const REG_KEY = "sp.activeTransfers.v1";

function loadRegistry(): ActiveRegistry {
  try {
    const raw = localStorage.getItem(REG_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") return parsed as ActiveRegistry;
  } catch {
    // ignore
  }
  return {};
}

function saveRegistry(map: ActiveRegistry) {
  try {
    localStorage.setItem(REG_KEY, JSON.stringify(map));
  } catch {
    // ignore
  }
}

function registerActive(id: string, kind: TransferKind) {
  const map = loadRegistry();
  if (map[id] !== kind) {
    map[id] = kind;
    saveRegistry(map);
  }
}

function unregisterActive(id: string) {
  const map = loadRegistry();
  if (id in map) {
    delete map[id];
    saveRegistry(map);
  }
}

async function refreshUploadStatus(id: string) {
  const status = await nv
    .upload_status(id)
    .unwrapOr(null as unknown as UploadStatus | null);
  if (!status) return;
  const store = useTransferStore.getState();
  // Ensure basic item exists
  store.update(id, {
    id,
    type: "upload",
    key: status.key,
    bytesTotal: status.bytes_total,
    bytesDone: status.bytes_done,
    rateBps: status.rate_bps ?? 0,
  });
}

async function refreshDownloadStatus(id: string) {
  const status = await nv
    .download_status(id)
    .unwrapOr(null as unknown as DownloadStatus | null);
  if (!status) return;
  const store = useTransferStore.getState();
  store.update(id, {
    id,
    type: "download",
    key: status.key,
    bytesTotal: status.bytes_total,
    bytesDone: status.bytes_done,
    rateBps: status.rate_bps ?? 0,
  });
}

function handleUploadEvent(ev: UploadEvent) {
  const id = ev.transfer_id;
  const s = useTransferStore.getState();
  switch (ev.type) {
    case "Started": {
      registerActive(id, "upload");
      s.update(id, {
        id,
        type: "upload",
        bytesDone: 0,
        rateBps: 0,
        state: "running",
      });
      // Defer heavy status fetch
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "Resumed": {
      s.update(id, { state: "running" });
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "Paused": {
      s.update(id, { state: "paused" });
      break;
    }
    case "PartDone": {
      // Keep status as source of truth
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "Completed": {
      s.update(id, { state: "completed" });
      unregisterActive(id);
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
    case "Failed": {
      s.update(id, { state: "failed", error: ev.error?.message ?? "failed" });
      unregisterActive(id);
      setTimeout(() => void refreshUploadStatus(id), 0);
      break;
    }
  }
}

async function handleDownloadEvent(ev: DownloadEvent) {
  const id = ev.transfer_id;
  const s = useTransferStore.getState();
  console.log("handleDownloadEvent", ev);
  switch (ev.type) {
    case "Started": {
      registerActive(id, "download");
      s.update(id, {
        id,
        type: "download",
        bytesDone: 0,
        rateBps: 0,
        state: "running",
      });
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "Resumed": {
      s.update(id, { state: "running" });
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "Paused": {
      s.update(id, { state: "paused" });
      break;
    }
    case "ChunkDone": {
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "Completed": {
      s.update(id, { state: "completed" });
      // Post-process: if this download has a tempPath and final destPath, move/copy it now
      try {
        const item = useTransferStore.getState().items[id];
        const src = item?.tempPath;
        const dst = item?.destPath;
        if (src && dst) {
          console.log("copying from: ", src, "to: ", dst);
          const srcF = await open(src, { read: true });
          const dstF = await open(dst, {
            write: true,
            create: true,
            truncate: true,
          });

          const buf = new Uint8Array(1024 * 1024);
          let off = 0;
          let totalW = 0;
          console.log("stream copying, buffer length", buf.length);
          for (;;) {
            const read = await srcF.read(buf);
            if (read === null) break;
            await dstF.write(buf.subarray(0, read));
            off += read;
            totalW += read;
          }
          console.log("stream copying, off", off);
          // Some providers require explicit truncate to finalize size metadata
          try {
            await dstF.truncate(off);
          } catch {}
          await srcF.close();
          console.log("stream copying, srcF closed");
          await dstF.close();
          console.log("stream copying, dstF closed");

          const meta = await stat(dst); // verify size
          console.log(
            "wrote=",
            totalW,
            "dst.size=",
            meta.size,
            "readonly?",
            meta.readonly,
          );
          // best-effort cleanup of sandbox temp
          try {
            await remove(src);
          } catch {}
        }
      } catch (e) {
        console.error("post-download move failed", e);
        s.update(id, { error: "move failed" });
      }
      unregisterActive(id);
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "Failed": {
      s.update(id, { state: "failed", error: ev.error?.message ?? "failed" });
      unregisterActive(id);
      setTimeout(() => void refreshDownloadStatus(id), 0);
      break;
    }
    case "SourceChanged": {
      s.update(id, { state: "failed", error: "SourceChanged" });
      unregisterActive(id);
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
      poller?: number;
    };
  };
  if (w.__SP_EVENT_BRIDGE__?.initialized) return;
  const ctrl: NonNullable<typeof w.__SP_EVENT_BRIDGE__> = {
    initialized: true,
    unlistenUpload: undefined,
    unlistenDownload: undefined,
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

  // Recover from persisted registry on startup
  const reg = loadRegistry();
  const entries = Object.entries(reg);
  if (entries.length > 0) {
    for (const [id, kind] of entries) {
      if (kind === "upload") {
        // Seed store as running/pending, then refresh
        useTransferStore.getState().update(id, {
          id,
          type: "upload",
          bytesDone: 0,
          rateBps: 0,
          state: "running",
        });
        // Best-effort status fetch
        setTimeout(() => void refreshUploadStatus(id), 0);
      } else {
        useTransferStore.getState().update(id, {
          id,
          type: "download",
          bytesDone: 0,
          rateBps: 0,
          state: "running",
        });
        setTimeout(() => void refreshDownloadStatus(id), 0);
      }
    }
  }

  // Seed logger status and initial tail once
  try {
    const st = await nv.log_get_status().unwrapOr(undefined as any);
    if (st) useLogStore.getState().setStatus(st as any);
    const tail = await nv.log_tail(400).unwrapOr("");
    if (tail) useLogStore.getState().setAll(String(tail).split("\n"));
  } catch {}

  // Load app settings to seed default download directory
  try {
    const s = await nv.settings_get().unwrapOr(undefined as any);
    const dir = s?.defaultDownloadDir as string | undefined;
    if (dir && String(dir).trim().length > 0) {
      useAppStore.getState().setDefaultDownloadDir(String(dir).trim());
    }
  } catch {}

  // Poll active transfers periodically to stay in sync (idempotent)
  ctrl.poller = window.setInterval(async () => {
    const active = loadRegistry();
    const ids = Object.keys(active);
    console.log("polling");
    if (ids.length === 0) return;
    // Limit per tick to avoid bursts
    const batch = ids.slice(0, 6);
    await Promise.all(
      batch.map((id) =>
        active[id] === "upload"
          ? refreshUploadStatus(id)
          : refreshDownloadStatus(id),
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
