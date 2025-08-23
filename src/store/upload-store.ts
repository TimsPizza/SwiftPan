import { create } from "zustand";

export type UploadStatus =
  | "pending"
  | "uploading"
  | "paused"
  | "success"
  | "error";

export interface UploadItem {
  id: string; // A unique ID for the upload item, e.g., fileId or a temporary UID
  sessionId: string; // upload session ID
  file: File;
  progress: number; // 0-100 (for backward compatibility)
  clientProgress: number; // 0-100 (client direct upload progress - blue)
  confirmProgress: number; // 0-100 (server confirmation progress - green)
  status: UploadStatus;
  speed: number; // bytes per second
  error?: string;
}

interface UploadState {
  uploads: UploadItem[];
  addUpload: (item: UploadItem) => void;
  updateUploadProgress: (id: string, progress: number, speed: number) => void;
  updateTwoStageProgress: (
    id: string,
    clientProgress: number,
    confirmProgress: number,
    speed?: number,
  ) => void;
  setUploadStatus: (id: string, status: UploadStatus, error?: string) => void;
  removeUpload: (id: string) => void;
  clearCompleted: () => void;
}

export const useUploadStore = create<UploadState>((set) => ({
  uploads: [],
  addUpload: (item) => set((state) => ({ uploads: [...state.uploads, item] })),
  updateUploadProgress: (id, progress, speed) =>
    set((state) => ({
      uploads: state.uploads.map((u) =>
        u.id === id ? { ...u, progress, speed, status: "uploading" } : u,
      ),
    })),
  updateTwoStageProgress: (id, clientProgress, confirmProgress, speed = 0) =>
    set((state) => ({
      uploads: state.uploads.map((u) =>
        u.id === id
          ? {
              ...u,
              clientProgress,
              confirmProgress,
              progress: Math.max(clientProgress, confirmProgress), // backward compatibility
              speed,
              status: "uploading",
            }
          : u,
      ),
    })),
  setUploadStatus: (id, status, error) =>
    set((state) => ({
      uploads: state.uploads.map((u) =>
        u.id === id
          ? {
              ...u,
              status,
              error,
              progress: status === "success" ? 100 : u.progress,
              clientProgress:
                status === "success" ? 100 : u.clientProgress || u.progress,
              confirmProgress:
                status === "success" ? 100 : u.confirmProgress || 0,
            }
          : u,
      ),
    })),
  removeUpload: (id) =>
    set((state) => ({ uploads: state.uploads.filter((u) => u.id !== id) })),
  clearCompleted: () =>
    set((state) => ({
      uploads: state.uploads.filter((u) => u.status !== "success"),
    })),
}));
