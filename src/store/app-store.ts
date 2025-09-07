import { create } from "zustand";

export interface AppSettingsState {
  // Mirrors backend AppSettings (camelCase)
  logLevel: string;
  maxConcurrency: number;
  defaultDownloadDir: string | null;
  uploadThumbnail: boolean;
  androidTreeUri?: string | null;

  // Mutators
  setSettings: (s: Partial<AppSettingsState>) => void;
  setLogLevel: (v: string) => void;
  setMaxConcurrency: (v: number) => void;
  setDefaultDownloadDir: (dir: string | null) => void;
  setUploadThumbnail: (v: boolean) => void;
  setAndroidTreeUri: (v: string | null) => void;
}

export const useAppStore = create<AppSettingsState>((set) => ({
  logLevel: "info",
  maxConcurrency: 2,
  defaultDownloadDir: null,
  uploadThumbnail: false,
  androidTreeUri: null,

  setSettings: (s) =>
    set((state) => ({
      ...state,
      ...s,
    })),
  setLogLevel: (v) => set(() => ({ logLevel: v })),
  setMaxConcurrency: (v) => set(() => ({ maxConcurrency: v })),
  setDefaultDownloadDir: (dir) =>
    set(() => ({ defaultDownloadDir: dir ?? null })),
  setUploadThumbnail: (v) => set(() => ({ uploadThumbnail: !!v })),
  setAndroidTreeUri: (v) => set(() => ({ androidTreeUri: v })),
}));
