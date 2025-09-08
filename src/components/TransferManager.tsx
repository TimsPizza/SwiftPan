import {
  Drawer,
  DrawerContent,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { Progress } from "@/components/ui/progress";
import { SpError } from "@/lib/api/bridge";
import { nv } from "@/lib/api/tauriBridge";
import { formatBytes } from "@/lib/utils";
import { useTransferStore } from "@/store/transfer-store";
import { useMemo, useState } from "react";

export default function TransferManager() {
  const items = useTransferStore((s) => s.items);
  const open = useTransferStore((s) => s.ui.open);
  const setOpen = useTransferStore((s) => s.ui.setOpen);
  const [busy, setBusy] = useState<Set<string>>(new Set());

  const sortedItems = useMemo(
    () => Object.values(items).sort((a, b) => a.id.localeCompare(b.id)),
    [items],
  );

  const setBusyFor = (id: string, v: boolean) => {
    setBusy((prev) => {
      const next = new Set(prev);
      if (v) next.add(id);
      else next.delete(id);
      return next;
    });
  };

  const ctrl = async (
    id: string,
    type: "upload" | "download",
    action: "pause" | "resume" | "cancel",
  ) => {
    if (busy.has(id)) return;
    setBusyFor(id, true);
    try {
      if (type === "upload") {
        await nv.upload_ctrl(id, action);
      } else {
        const r = nv.download_ctrl(id, action);
        await r.match(
          () => {},
          (e) => {
            const err = e as SpError;
            if (action === "cancel" && err.message.search("not found")) {
              useTransferStore
                .getState()
                .update(id, { state: "failed", error: err.message });
            }
          },
        );
      }
    } finally {
      setBusyFor(id, false);
    }
  };
  return (
    <Drawer open={open} onOpenChange={setOpen}>
      <DrawerContent side="right">
        <DrawerHeader>
          <DrawerTitle>Task Manager</DrawerTitle>
        </DrawerHeader>
        <div className="max-h-[calc(100vh-8rem)] overflow-auto p-2 text-xs">
          {sortedItems.length === 0 ? (
            <div className="text-muted-foreground p-3">No active tasks</div>
          ) : (
            <ul className="space-y-2">
              {sortedItems.map((t) => (
                <li
                  key={t.id}
                  className="flex items-center justify-between gap-2 rounded border p-2"
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">
                      {t.key || t.id}
                    </div>
                    <div className="text-muted-foreground text-[10px]">
                      {t.type} · {formatBytes(t.bytesDone)}
                      {t.bytesTotal
                        ? ` / ${formatBytes(t.bytesTotal)}`
                        : ""} · {t.state}
                    </div>
                    <div className="mt-1">
                      <Progress
                        value={
                          t.bytesTotal
                            ? Math.min(
                                100,
                                Math.floor(
                                  (t.bytesDone / (t.bytesTotal || 1)) * 100,
                                ),
                              )
                            : 0
                        }
                      />
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    {t.state === "running" ? (
                      <button
                        className="hover:bg-muted rounded px-2 py-1 text-[11px]"
                        disabled={busy.has(t.id)}
                        onClick={() => ctrl(t.id, t.type, "pause")}
                        aria-label="Pause"
                      >
                        Pause
                      </button>
                    ) : t.state === "paused" ? (
                      <button
                        className="hover:bg-muted rounded px-2 py-1 text-[11px]"
                        disabled={busy.has(t.id)}
                        onClick={() => ctrl(t.id, t.type, "resume")}
                        aria-label="Resume"
                      >
                        Resume
                      </button>
                    ) : (
                      <span className="text-muted-foreground text-[11px]">
                        {t.state}
                      </span>
                    )}
                    {(t.state === "running" || t.state === "paused") && (
                      <button
                        className="hover:bg-muted rounded px-2 py-1 text-[11px]"
                        disabled={busy.has(t.id)}
                        onClick={() => ctrl(t.id, t.type, "cancel")}
                        aria-label="Cancel"
                      >
                        Cancel
                      </button>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </DrawerContent>
    </Drawer>
  );
}
