import { create } from "zustand";
import type { FileItem as File } from "@/lib/api/schemas";

type FilesStore = {
  files: File[];
  setFiles: (files: File[]) => void;
  upsertFiles: (files: File[]) => void;
  removeFiles: (ids: string[]) => void;
  getFile: (id: string) => File | undefined;
};

function dedupeFiles(list: File[]): File[] {
  const seen = new Map<string, File>();
  for (const file of list) {
    if (!file?.id) continue;
    seen.set(file.id, file);
  }
  return Array.from(seen.values());
}

export const useFilesStore = create<FilesStore>((set, get) => ({
  files: [],
  setFiles: (files) => set({ files: dedupeFiles(files) }),
  upsertFiles: (incoming) => {
    if (!incoming || incoming.length === 0) return;
    const merged = new Map<string, File>();
    for (const file of get().files) {
      merged.set(file.id, file);
    }
    for (const file of incoming) {
      if (!file?.id) continue;
      merged.set(file.id, file);
    }
    set({ files: Array.from(merged.values()) });
  },
  removeFiles: (ids) => {
    if (!ids || ids.length === 0) return;
    const removeSet = new Set(ids);
    set({ files: get().files.filter((file) => !removeSet.has(file.id)) });
  },
  getFile: (id) => get().files.find((file) => file.id === id),
}));
