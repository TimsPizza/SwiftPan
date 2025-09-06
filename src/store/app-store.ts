import { create } from "zustand";

interface AppStoreState {
  defaultDownloadDir: string | null;
  setDefaultDownloadDir: (dir: string) => void;
}

export const useAppStore = create<AppStoreState>((set) => ({
  defaultDownloadDir: null,
  setDefaultDownloadDir: (dir: string) => set(() => ({ defaultDownloadDir: dir })),
}));

