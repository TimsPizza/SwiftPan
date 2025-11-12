import type { FileItem as File } from "@/lib/api/schemas";
import { api } from "@/lib/api/tauriBridge";
// import { useAppStore } from "@/store/app-store";
// import { useTransferStore } from "@/store/transfer-store";
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
  deleteSelectedOrByFileId: (
    id?: string,
  ) => Promise<{ successIds: string[]; failedIds: string[] }>;
  getSelectedFiles: () => File[];
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
  // download/upload concerns moved to platform-specific hooks
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
  }, [
    files,
    removedIds,
    search,
    timeFilter,
    minSizeMB,
    maxSizeMB,
    typeFilter,
    getCategory,
  ]);
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
          const okId = await api.delete_object(id);
          const k = String(okId ?? id);
          successIds.push(k);
          setRemovedIds((prev) => new Set(prev).add(k));
          setSelectedIds((prev) => {
            const next = new Set(prev);
            next.delete(k);
            return next;
          });
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
    deleteSelectedOrByFileId,
    getSelectedFiles,
    selectAll,
    deselectAll,
    sortBy,
    sortOrder,
    toggleSort,
    setSortBy,
    setSortOrder,
  };
};
