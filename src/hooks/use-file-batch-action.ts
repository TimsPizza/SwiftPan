import { fileApi } from "@/lib/api";
import type { File } from "@/lib/api/schemas";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "react-query";
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
  const queryClient = useQueryClient();
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // map for quick id -> file lookup
  const idToFile = useMemo(() => {
    const m = new Map<string, File>();
    (files || []).forEach((f) => m.set(f.id, f));
    return m;
  }, [files]);

  // compute visible ids (filtered + not deleted)
  const visibleIds = useMemo(
    () => (visibleFiles || []).filter((f) => !f.deletedAt).map((f) => f.id),
    [visibleFiles],
  );

  const allVisibleSelected = useMemo(
    () =>
      visibleIds.length > 0 && visibleIds.every((id) => selectedIds.has(id)),
    [visibleIds, selectedIds],
  );

  const selectedCount = selectedIds.size;

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
        const url = await fileApi.getDownloadUrl(f.id);
        // Fetch the file data to avoid cross-origin navigation
        const resp = await fetch(url, { mode: "cors" });
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        const blob = await resp.blob();
        const objectUrl = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = objectUrl;
        a.download = f.filename || "download";
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        // Revoke after a short delay to ensure download starts
        setTimeout(() => URL.revokeObjectURL(objectUrl), 5000);
        // queue sequentially to avoid multiple-download throttling
        await delay(400);
      } catch (e) {
        toast.error(`Failed to download ${f.filename}`);
      }
    }
  }, [selectedIds, idToFile]);

  const downloadOne = useCallback(async (file: File) => {
    try {
      const url = await fileApi.getDownloadUrl(file.id);
      const resp = await fetch(url, { mode: "cors" });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const blob = await resp.blob();
      const objectUrl = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = objectUrl;
      a.download = file.filename || "download";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      setTimeout(() => URL.revokeObjectURL(objectUrl), 5000);
    } catch (e) {
      toast.error(`Failed to download ${file.filename}`);
    }
  }, []);

  const deleteSelected = useCallback(async () => {
    if (selectedIds.size === 0) return { success: 0, failed: 0 };
    let success = 0;
    let failed = 0;
    for (const id of Array.from(selectedIds)) {
      try {
        await fileApi.deleteFile(id);
        success++;
      } catch (e) {
        failed++;
      }
    }
    if (success > 0) toast.success(`Deleted ${success} file(s)`);
    if (failed > 0) toast.error(`Failed to delete ${failed} file(s)`);
    clearSelection();
    queryClient.invalidateQueries("files");
    return { success, failed };
  }, [selectedIds, clearSelection, queryClient]);

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
