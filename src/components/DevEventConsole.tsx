import { useEffect, useState } from "react";
import { onUploadEvent, onDownloadEvent, onBackgroundStats } from "@/lib/api/tauriBridge";

type Item = { ts: number; kind: string; data: unknown };

export default function DevEventConsole() {
  const [items, setItems] = useState<Item[]>([]);

  useEffect(() => {
    const add = (kind: string, data: unknown) => {
      setItems((prev) => [{ ts: Date.now(), kind, data }, ...prev].slice(0, 20));
    };
    let unsubs: Array<() => void> = [];
    (async () => {
      unsubs.push(await onUploadEvent((p) => add("upload", p)));
      unsubs.push(await onDownloadEvent((p) => add("download", p)));
      unsubs.push(await onBackgroundStats((p) => add("bg", p)));
    })();
    return () => {
      unsubs.forEach((u) => u());
    };
  }, []);

  if (items.length === 0) return null;
  return (
    <div className="fixed left-4 bottom-4 z-50 w-[420px] max-h-80 overflow-auto rounded border bg-white p-2 text-xs shadow">
      <div className="mb-1 font-semibold">Tauri Events</div>
      {items.map((it, idx) => (
        <div key={idx} className="border-t py-1">
          <div className="text-[10px] text-gray-500">{new Date(it.ts).toLocaleTimeString()} · {it.kind}</div>
          <pre className="whitespace-pre-wrap break-words">{JSON.stringify(it.data)}</pre>
        </div>
      ))}
    </div>
  );
}

