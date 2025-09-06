import type { FileItem as File } from "@/lib/api/schemas";
import { nv, queries } from "@/lib/api/tauriBridge";
import { useTransferStore } from "@/store/transfer-store";
import { downloadDir, join } from "@tauri-apps/api/path";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

export interface UseFileBatchActionReturn {
  selectedIds: Set<string>;
  selectedCount: number;
  allVisibleSelected: boolean;
  toggleOne: (id: string) => void;
  toggleAllVisible: () => void;
  clearSelection: () => void;
  batchDownload: () => Promise<void>;
  deleteSelected: () => Promise<{ success: number; failed: number }>;
  getSelectedFiles: () => File[];
  downloadOne: (file: File) => Promise<void>;
}

export const useFileBatchAction = (
  files: File[] | undefined,
  visibleFiles: File[] | undefined,
): UseFileBatchActionReturn => {
  const settingsQ = queries.useSettings();
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // map for quick id -> file lookup
  const idToFile = useMemo(() => {
    const m = new Map<string, File>();
    (files || []).forEach((f) => m.set(f.id, f));
    return m;
  }, [files]);

  // compute visible ids (filtered + not deleted)
  const visibleIds = useMemo(
    () => (visibleFiles || []).map((f) => f.id),
    [visibleFiles],
  );

  const allVisibleSelected = useMemo(
    () =>
      visibleIds.length > 0 && visibleIds.every((id) => selectedIds.has(id)),
    [visibleIds, selectedIds],
  );

  const selectedCount = selectedIds.size;
  // Global download events handled by EventBridge

  // prune selection when files change
  useEffect(() => {
    if (!files) return;
    const valid = new Set(files.map((f) => f.id));
    setSelectedIds((prev) => {
      const next = new Set<string>();
      prev.forEach((id) => {
        if (valid.has(id)) next.add(id);
      });
      return next;
    });
  }, [files]);

  const toggleOne = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const toggleAllVisible = useCallback(() => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      const everySelected = visibleIds.every((id) => next.has(id));
      if (everySelected) {
        visibleIds.forEach((id) => next.delete(id));
      } else {
        visibleIds.forEach((id) => next.add(id));
      }
      return next;
    });
  }, [visibleIds]);

  const clearSelection = useCallback(() => setSelectedIds(new Set()), []);

  const batchDownload = useCallback(async () => {
    const targets: File[] = Array.from(selectedIds)
      .map((id) => idToFile.get(id))
      .filter(Boolean) as File[];

    const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

    for (const f of targets) {
      try {
        const base =
          (settingsQ.data?.defaultDownloadDir as string | undefined) ||
          (await downloadDir());
        const dest = await join(base, f.filename || "download");
        // prevent duplicate active downloads for same key
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
        const r = await nv.download_new({
          key: f.id,
          dest_path: dest,
          chunk_size: 4 * 1024 * 1024,
        });
        r.match(
          () => {
            useTransferStore.getState().ui.setOpen(true);
          },
          (e) => {
            throw new Error(String((e as any)?.message || e));
          },
        );
        // small delay to avoid UI jank
        await delay(400);
      } catch (e) {
        toast.error(`Failed to download ${f.filename}`);
      }
    }
  }, [selectedIds, idToFile, settingsQ.data]);

  const downloadOne = useCallback(
    async (file: File) => {
      try {
        const base =
          (settingsQ.data?.defaultDownloadDir as string | undefined) ||
          (await downloadDir());
        const dest = await join(base, file.filename || "download");
        const active = useTransferStore.getState().items;
        const dup = Object.values(active).some(
          (t) =>
            t.type === "download" &&
            t.key === file.id &&
            t.state !== "completed" &&
            t.state !== "failed",
        );
        if (dup) {
          toast.info(`Already downloading: ${file.filename}`);
          return;
        }
        const r = await nv.download_new({
          key: file.id,
          dest_path: dest,
          chunk_size: 4 * 1024 * 1024,
        });
        r.match(
          () => {
            useTransferStore.getState().ui.setOpen(true);
          },
          (e) => {
            throw new Error(String((e as any)?.message || e));
          },
        );
      } catch (e) {
        toast.error(`Failed to download ${file.filename}`);
      }
    },
    [settingsQ.data],
  );

  const deleteSelected = useCallback(async () => {
    if (selectedIds.size === 0) return { success: 0, failed: 0 };
    let success = 0;
    let failed = 0;
    for (const id of Array.from(selectedIds)) {
      try {
        const r = await nv.delete_object(id);
        r.match(
          () => success++,
          () => failed++,
        );
      } catch (e) {
        failed++;
      }
    }
    if (success > 0) toast.success(`Deleted ${success} file(s)`);
    if (failed > 0) toast.error(`Failed to delete ${failed} file(s)`);
    clearSelection();
    return { success, failed };
  }, [selectedIds, clearSelection]);

  const getSelectedFiles = useCallback((): File[] => {
    return Array.from(selectedIds)
      .map((id) => idToFile.get(id))
      .filter(Boolean) as File[];
  }, [selectedIds, idToFile]);

  return {
    selectedIds,
    selectedCount,
    allVisibleSelected,
    toggleOne,
    toggleAllVisible,
    clearSelection,
    batchDownload,
    deleteSelected,
    getSelectedFiles,
    downloadOne,
  };
};
