import type { FileItem as File } from "@/lib/api/schemas";
import { api } from "@/lib/api/tauriBridge";
import { useAppStore } from "@/store/app-store";
import { useFilesStore } from "@/store/files-store";
import { useTransferStore } from "@/store/transfer-store";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import { toast } from "sonner";

function joinPath(base: string, name: string) {
  if (!base) return name;
  const trimmed =
    base.endsWith("/") || base.endsWith("\\") ? base.slice(0, -1) : base;
  const useBackslash = trimmed.includes("\\");
  const sep = useBackslash ? "\\" : "/";
  return `${trimmed}${sep}${name}`;
}

export function useDesktopFileTransfer() {
  const setTransfersOpen = useTransferStore((s) => s.ui.setOpen);
  const defaultBase = useAppStore((s) => s.defaultDownloadDir);
  const setDefaultBase = useAppStore((s) => s.setDefaultDownloadDir);
  const files = useFilesStore((s) => s.files);

  const persistDefaultBase = useCallback(
    async (chosen: string) => {
      try {
        setDefaultBase(chosen);
        const app = await api.settings_get();
        if (app) {
          await api.settings_set({ ...app, defaultDownloadDir: chosen });
        }
      } catch {
        /* ignore */
      }
    },
    [setDefaultBase],
  );

  const pickUploads = useCallback(async () => {
    const picked = await open({ multiple: true });
    const entries = picked ? (Array.isArray(picked) ? picked : [picked]) : [];
    if (entries.length === 0) return;
    const selected = entries.map((e) => String(e));
    const bucketKeys = new Set((files || []).map((f) => f.id));
    const storeItems = useTransferStore.getState().items;
    for (const filePath of selected) {
      const fileName = filePath.split("/").pop() || filePath;
      const key = fileName;
      if (bucketKeys.has(key)) {
        toast.error(`File already exists: ${key}`);
        continue;
      }
      const dup = Object.values(storeItems).some(
        (t) =>
          t.type === "upload" &&
          t.key === key &&
          t.state !== "completed" &&
          t.state !== "failed",
      );
      if (dup) {
        toast.info(`Already uploading: ${key}`);
        continue;
      }
      try {
        await api.upload_new({
          key,
          source_path: filePath,
          part_size: 8 * 1024 * 1024,
        });
        setTransfersOpen(true);
      } catch (e) {
        console.error(e);
        toast.error(`Upload failed to start: ${fileName}`);
      }
    }
  }, [files, setTransfersOpen]);

  const downloadOne = useCallback(
    async (file: File) => {
      const base = defaultBase;
      const defaultPath =
        base && base.trim().length > 0
          ? `${base.replace(/[\\/]$/, "")}/${file.filename}`
          : file.filename;
      const picked = await save({ defaultPath });
      if (!picked) return;
      const destPath = String(picked);
      try {
        await api.download_new({
          key: file.id,
          dest_path: destPath,
          chunk_size: 4 * 1024 * 1024,
        });
        setTransfersOpen(true);
      } catch (e) {
        console.error(e);
        toast.error(`Failed to start download: ${file.filename}`);
      }
    },
    [defaultBase, setTransfersOpen],
  );

  const resolveBaseDir = useCallback(async (): Promise<string | null> => {
    // Always prompt user to pick a target directory for batch downloads.
    // If a default base exists, use it as the preselected path.
    const picked = await open({
      directory: true,
      multiple: false,
      defaultPath: defaultBase || undefined,
    });
    if (!picked) return null;
    const base = String(picked);
    // Persist as default only if none was previously set (preserves existing preference behavior).
    if (!defaultBase && base) await persistDefaultBase(base);
    return base || null;
  }, [defaultBase, persistDefaultBase]);

  const downloadMany = useCallback(
    async (many: File[]) => {
      if (!many || many.length === 0) return;
      const base = await resolveBaseDir();
      if (!base) return;
      for (const f of many) {
        try {
          const dest = joinPath(base, f.filename || "download");
          const active = useTransferStore.getState().items;
          const dup = Object.values(active).some(
            (t) =>
              t.type === "download" &&
              t.key === f.id &&
              t.state !== "completed" &&
              t.state !== "failed",
          );
          if (dup) {
            toast.info(`Already downloading: ${f.filename}`);
            continue;
          }
          await api.download_new({
            key: f.id,
            dest_path: dest,
            chunk_size: 4 * 1024 * 1024,
          });
          setTransfersOpen(true);
          await new Promise((r) => setTimeout(r, 200));
        } catch (e) {
          console.error(e);
          toast.error(`Failed to download ${f.filename}`);
        }
      }
    },
    [resolveBaseDir, setTransfersOpen],
  );

  return { pickUploads, downloadOne, downloadMany } as const;
}
