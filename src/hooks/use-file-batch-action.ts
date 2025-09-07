import type { FileItem as File } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { useAppStore } from "@/store/app-store";
import { useTransferStore } from "@/store/transfer-store";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

export interface UseFileBatchActionReturn {
  allFiles: File[];
  // pagination
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
  pageFiles: File[];
  nextPage: () => void;
  prevPage: () => void;
  setPage: (n: number) => void;
  setPageSize: (n: number) => void;
  // filtering
  search: string;
  setSearch: (q: string) => void;
  typeFilter: "all" | "image" | "video" | "audio" | "doc" | "other";
  setTypeFilter: (
    t: "all" | "image" | "video" | "audio" | "doc" | "other",
  ) => void;
  timeFilter: "any" | "24h" | "7d" | "30d";
  setTimeFilter: (t: "any" | "24h" | "7d" | "30d") => void;
  minSizeMB: string;
  maxSizeMB: string;
  setMinSizeMB: (v: string) => void;
  setMaxSizeMB: (v: string) => void;
  selectedIds: Set<string>;
  selectedCount: number;
  allVisibleSelected: boolean;
  toggleOne: (id: string) => void;
  toggleAllVisible: () => void;
  clearSelection: () => void;
  batchDownload: (destBase?: string) => Promise<void>;
  deleteSelectedOrByFileId: (
    id?: string,
  ) => Promise<{ successIds: string[]; failedIds: string[] }>;
  getSelectedFiles: () => File[];
  downloadOne: (file: File, destBase?: string) => Promise<void>;
  selectAll: () => void;
  deselectAll: () => void;
  // sorting helpers
  sortBy: "name" | "size" | "uploadedAt" | "type" | null;
  sortOrder: "asc" | "desc";
  toggleSort: (key: "name" | "size" | "uploadedAt" | "type") => void;
  setSortBy: (k: "name" | "size" | "uploadedAt" | "type" | null) => void;
  setSortOrder: (o: "asc" | "desc") => void;
}

export const useFileBatchAction = (
  files: File[] | undefined,
): UseFileBatchActionReturn => {
  // Use app store as source of download base directory initialized at startup
  const defaultBase = useAppStore((s) => s.defaultDownloadDir);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [removedIds, setRemovedIds] = useState<Set<string>>(new Set());

  // map for quick id -> file lookup
  const idToFile = useMemo(() => {
    const m = new Map<string, File>();
    (files || []).forEach((f) => m.set(f.id, f));
    return m;
  }, [files]);

  // filtering + sorting + pagination
  const [search, setSearch] = useState<string>("");
  const [typeFilter, setTypeFilter] = useState<
    "all" | "image" | "video" | "audio" | "doc" | "other"
  >("all");
  const [timeFilter, setTimeFilter] = useState<"any" | "24h" | "7d" | "30d">(
    "any",
  );
  const [minSizeMB, setMinSizeMB] = useState<string>("");
  const [maxSizeMB, setMaxSizeMB] = useState<string>("");
  const [sortBy, setSortBy] = useState<
    "name" | "size" | "uploadedAt" | "type" | null
  >(null);
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("desc");
  const getCategory = useCallback((filename: string, mimeType?: string) => {
    const ext = filename.split(".").pop()?.toLowerCase();
    if (
      ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff"].includes(
        ext || "",
      )
    )
      return "image" as const;
    if (["mp4", "avi", "mov", "wmv", "mkv", "webm"].includes(ext || ""))
      return "video" as const;
    if (["mp3", "wav", "flac", "aac", "ogg", "m4a"].includes(ext || ""))
      return "audio" as const;
    if (
      [
        "pdf",
        "doc",
        "docx",
        "txt",
        "md",
        "xls",
        "xlsx",
        "ppt",
        "pptx",
      ].includes(ext || "")
    )
      return "doc" as const;
    if (mimeType?.startsWith("image/")) return "image" as const;
    if (mimeType?.startsWith("video/")) return "video" as const;
    if (mimeType?.startsWith("audio/")) return "audio" as const;
    return "other" as const;
  }, []);
  const toggleSort = useCallback(
    (key: "name" | "size" | "uploadedAt" | "type") => {
      setSortBy((prev) => {
        if (prev !== key) {
          setSortOrder("asc");
          return key;
        }
        setSortOrder((o) => (o === "asc" ? "desc" : "asc"));
        return prev;
      });
    },
    [],
  );
  const sortFiles = useCallback(
    (arr: File[]): File[] => {
      if (!sortBy) return arr;
      const dir = sortOrder === "asc" ? 1 : -1;
      const copy = [...arr];
      copy.sort((a, b) => {
        switch (sortBy) {
          case "name":
            return dir * a.filename.localeCompare(b.filename);
          case "size":
            return dir * (a.size - b.size);
          case "uploadedAt":
            return dir * (a.uploadedAt - b.uploadedAt);
          case "type":
            return (
              dir *
              getCategory(a.filename, a.mimeType).localeCompare(
                getCategory(b.filename, b.mimeType),
              )
            );
          default:
            return 0;
        }
      });
      return copy;
    },
    [sortBy, sortOrder, getCategory],
  );
  const [page, setPage] = useState<number>(1);
  const [pageSize, setPageSize] = useState<number>(10);
  // Filter -> Sort -> Paginate
  const filteredAll: File[] = useMemo(() => {
    const q = search.trim().toLowerCase();
    const now = Date.now();
    let startTime = 0;
    if (timeFilter === "24h") startTime = now - 24 * 60 * 60 * 1000;
    else if (timeFilter === "7d") startTime = now - 7 * 24 * 60 * 60 * 1000;
    else if (timeFilter === "30d") startTime = now - 30 * 24 * 60 * 60 * 1000;

    const minBytes = minSizeMB
      ? Math.max(0, Math.floor(Number(minSizeMB) * 1024 * 1024))
      : 0;
    const maxBytes = maxSizeMB
      ? Math.max(0, Math.floor(Number(maxSizeMB) * 1024 * 1024))
      : Number.POSITIVE_INFINITY;

    return (files || []).filter((f) => {
      if (!f) return false;
      if (removedIds.has(f.id)) return false;
      if (
        q &&
        !f.filename.toLowerCase().includes(q) &&
        !f.originalName.toLowerCase().includes(q)
      )
        return false;
      if (
        typeFilter !== "all" &&
        getCategory(f.filename, f.mimeType) !== typeFilter
      )
        return false;
      if (timeFilter !== "any" && f.uploadedAt < startTime) return false;
      if (f.size < minBytes || f.size > maxBytes) return false;
      return true;
    });
  }, [files, removedIds, search, timeFilter, minSizeMB, maxSizeMB, typeFilter, getCategory]);
  const totalItems = filteredAll.length;
  const totalPages = Math.max(1, Math.ceil(totalItems / pageSize));
  useEffect(() => {
    setPage((p) => Math.min(Math.max(1, p), totalPages));
  }, [totalPages]);
  const nextPage = useCallback(
    () => setPage((p) => Math.min(totalPages, p + 1)),
    [totalPages],
  );
  const prevPage = useCallback(() => setPage((p) => Math.max(1, p - 1)), []);
  const sortedAll = useMemo(
    () => sortFiles(filteredAll),
    [filteredAll, sortFiles],
  );
  const pageStart = (page - 1) * pageSize;
  const pageFiles = useMemo(
    () => sortedAll.slice(pageStart, pageStart + pageSize),
    [sortedAll, pageStart, pageSize],
  );
  const visibleIds = useMemo(() => pageFiles.map((f) => f.id), [pageFiles]);

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
    setRemovedIds((prev) => {
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

  const selectAll = useCallback(() => {
    if (!filteredAll || filteredAll.length === 0) return;
    setSelectedIds(new Set(filteredAll.map((f) => f.id)));
  }, [filteredAll]);
  const deselectAll = clearSelection;

  const resolveBase = useCallback(
    async (override?: string): Promise<string | null> => {
      if (override && override.trim().length > 0) return override;
      if (defaultBase && defaultBase.trim().length > 0)
        return defaultBase.trim();
      toast.error("Default download directory is not set");
      return null;
    },
    [defaultBase],
  );

  const batchDownload = useCallback(
    async (destBase?: string) => {
      const targets: File[] = Array.from(selectedIds)
        .map((id) => idToFile.get(id))
        .filter(Boolean) as File[];

      const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

      const base = await resolveBase(destBase);
      if (!base) return;

      // Helper to join base dir and filename without changing drive/root semantics
      const joinPath = (base: string, name: string) => {
        if (!base) return name;
        const trimmed =
          base.endsWith("/") || base.endsWith("\\") ? base.slice(0, -1) : base;
        const useBackslash = trimmed.includes("\\");
        const sep = useBackslash ? "\\" : "/";
        return `${trimmed}${sep}${name}`;
      };

      for (const f of targets) {
        try {
          const dest = joinPath(base, f.filename || "download");
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
    },
    [selectedIds, idToFile, resolveBase],
  );

  const downloadOne = useCallback(
    async (file: File, destBase?: string) => {
      try {
        const base = await resolveBase(destBase);
        if (!base) return;
        const joinPath = (b: string, n: string) => {
          const trimmed =
            b.endsWith("/") || b.endsWith("\\") ? b.slice(0, -1) : b;
          const useBackslash = trimmed.includes("\\");
          const sep = useBackslash ? "\\" : "/";
          return `${trimmed}${sep}${n}`;
        };
        const dest = joinPath(base, file.filename || "download");
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
    [resolveBase],
  );
  // if id is provided, delete only that file
  // if id is not provided, delete all selected files
  const deleteSelectedOrByFileId = useCallback(
    async (
      targetId?: string,
    ): Promise<{ successIds: string[]; failedIds: string[] }> => {
      const targets = targetId ? [targetId] : Array.from(selectedIds);
      if (targets.length === 0) return { successIds: [], failedIds: [] };
      const successIds: string[] = [];
      const failedIds: string[] = [];
      for (const id of targets) {
        try {
          const r = await nv.delete_object(id);
          r.match(
            (okId) => {
              const k = String(okId ?? id);
              successIds.push(k);
              setRemovedIds((prev) => new Set(prev).add(k));
              setSelectedIds((prev) => {
                const next = new Set(prev);
                next.delete(k);
                return next;
              });
            },
            () => failedIds.push(id),
          );
        } catch {
          failedIds.push(id);
        }
      }
      if (successIds.length > 0) {
        toast.success(
          successIds.length === 1
            ? `Deleted 1 file`
            : `Deleted ${successIds.length} files`,
        );
      }
      if (failedIds.length > 0) {
        toast.error(
          failedIds.length === 1
            ? `Failed to delete ${failedIds[0]}`
            : `Failed to delete ${failedIds.length} files`,
        );
      }
      return { successIds, failedIds };
    },
    [selectedIds],
  );

  const getSelectedFiles = useCallback((): File[] => {
    const files = Array.from(selectedIds)
      .map((id) => idToFile.get(id))
      .filter(Boolean) as File[];
    return files;
  }, [selectedIds, idToFile]);

  return {
    allFiles: files || [],
    page,
    pageSize,
    totalItems,
    totalPages,
    pageFiles,
    nextPage,
    prevPage,
    setPage,
    setPageSize,
    search,
    setSearch,
    typeFilter,
    setTypeFilter,
    timeFilter,
    setTimeFilter,
    minSizeMB,
    maxSizeMB,
    setMinSizeMB,
    setMaxSizeMB,
    selectedIds,
    selectedCount,
    allVisibleSelected,
    toggleOne,
    toggleAllVisible,
    clearSelection,
    batchDownload,
    deleteSelectedOrByFileId,
    getSelectedFiles,
    downloadOne,
    selectAll,
    deselectAll,
    sortBy,
    sortOrder,
    toggleSort,
    setSortBy,
    setSortOrder,
  };
};
