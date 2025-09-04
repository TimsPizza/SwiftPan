import { create } from "zustand";

export type TransferType = "upload" | "download";
export type TransferState =
  | "pending"
  | "running"
  | "paused"
  | "completed"
  | "failed";

export interface TransferItem {
  id: string; // transfer_id
  type: TransferType;
  key: string;
  destPath?: string;
  bytesTotal?: number;
  bytesDone: number;
  rateBps: number;
  state: TransferState;
  error?: string;
}

interface StoreState {
  items: Record<string, TransferItem>;
  upsert: (t: TransferItem) => void;
  update: (id: string, patch: Partial<TransferItem>) => void;
  remove: (id: string) => void;
  clearCompleted: () => void;
}

export const useTransferStore = create<StoreState>((set) => ({
  items: {},
  upsert: (t) => set((s) => ({ items: { ...s.items, [t.id]: t } })),
  update: (id, patch) =>
    set((s) => ({
      items: {
        ...s.items,
        [id]: {
          ...(s.items[id] ||
            ({ id, type: "download", key: "" } as TransferItem)),
          ...patch,
        },
      },
    })),
  remove: (id) =>
    set((s) => {
      const n = { ...s.items };
      delete n[id];
      return { items: n };
    }),
  clearCompleted: () =>
    set((s) => ({
      items: Object.fromEntries(
        Object.entries(s.items).filter(([, v]) => v.state !== "completed"),
      ),
    })),
}));
