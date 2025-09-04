import { useTransferStore } from "@/store/transfer-store";

export default function TransferManager() {
  const items = useTransferStore((s) => s.items);
  if (Object.keys(items).length === 0) return null;
  return (
    <div className="fixed right-4 bottom-4 z-40 max-h-80 w-[360px] overflow-auto rounded border bg-white p-2 text-xs shadow">
      <div className="mb-1 font-semibold">Transfers</div>
      <ul className="space-y-1">
        {Object.values(items).map((t) => (
          <li
            key={t.id}
            className="flex items-center justify-between gap-2 border-t py-1"
          >
            <div className="min-w-0 flex-1">
              <div className="truncate">
                {t.type}:{t.id}
              </div>
              <div className="text-[10px] text-gray-500">
                {t.bytesDone}/{t.bytesTotal ?? "?"} · {t.state}
              </div>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
