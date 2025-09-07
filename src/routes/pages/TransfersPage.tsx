import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { nv } from "@/lib/api/tauriBridge";
import { formatBytes } from "@/lib/utils";
import { useTransferStore } from "@/store/transfer-store";
import { useMemo, useState } from "react";

export default function TransfersPage() {
  const items = useTransferStore((s) => s.items);
  const remove = useTransferStore((s) => s.remove);
  const [busy, setBusy] = useState<Set<string>>(new Set());

  const sorted = useMemo(
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
      if (type === "upload") await nv.upload_ctrl(id, action);
      else await nv.download_ctrl(id, action);
    } finally {
      setBusyFor(id, false);
    }
  };

  const clearFinished = () => {
    for (const t of sorted) {
      if (t.state === "completed" || t.state === "failed") remove(t.id);
    }
  };

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Task Manager</CardTitle>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={clearFinished}>
              Clear completed
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {sorted.length === 0 ? (
            <div className="text-muted-foreground text-sm">No tasks</div>
          ) : (
            <ul className="space-y-2">
              {sorted.map((t) => {
                const pct = t.bytesTotal
                  ? Math.min(
                      100,
                      Math.floor((t.bytesDone / (t.bytesTotal || 1)) * 100),
                    )
                  : 0;
                const canRemove =
                  t.state === "completed" || t.state === "failed";
                return (
                  <li
                    key={t.id}
                    className="flex items-center justify-between gap-3 rounded border p-3"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium">
                        {t.key || t.id}
                      </div>
                      <div className="text-muted-foreground mt-0.5 text-[11px]">
                        {t.type} · {t.state} · {formatBytes(t.bytesDone)}
                        {t.bytesTotal ? ` / ${formatBytes(t.bytesTotal)}` : ""}
                      </div>
                      <div className="mt-1">
                        <Progress value={pct} />
                      </div>
                    </div>
                    <div className="flex items-center gap-1">
                      {t.state === "running" ? (
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={busy.has(t.id)}
                          onClick={() => ctrl(t.id, t.type, "pause")}
                        >
                          Pause
                        </Button>
                      ) : t.state === "paused" ? (
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={busy.has(t.id)}
                          onClick={() => ctrl(t.id, t.type, "resume")}
                        >
                          Resume
                        </Button>
                      ) : null}
                      {(t.state === "running" || t.state === "paused") && (
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={busy.has(t.id)}
                          onClick={() => ctrl(t.id, t.type, "cancel")}
                        >
                          Cancel
                        </Button>
                      )}
                      {canRemove && (
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => remove(t.id)}
                        >
                          Remove
                        </Button>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
