import { create } from "zustand";

export type LogEntry = {
  ts: string;
  level: string;
  target: string;
  message: string;
  line: string;
};

interface LogStatus {
  level: string;
  cache_lines: number;
  file_path: string;
  file_size_bytes: number;
}

interface LogStore {
  entries: LogEntry[];
  status?: LogStatus;
  setStatus: (s: LogStatus) => void;
  append: (e: LogEntry) => void;
  setAll: (lines: string[]) => void;
  clear: () => void;
}

export const useLogStore = create<LogStore>((set) => ({
  entries: [],
  status: undefined,
  setStatus: (s) => set({ status: s }),
  append: (e) => set((s) => ({ entries: [...s.entries.slice(-999), e] })),
  setAll: (lines) =>
    set({
      entries: lines
        .filter((l) => l.trim().length > 0)
        .map((line) => ({
          ts: "",
          level: "",
          target: "",
          message: line,
          line,
        })),
    }),
  clear: () => set({ entries: [] }),
}));
