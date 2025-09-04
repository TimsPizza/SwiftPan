import {
  Drawer,
  DrawerContent,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { useTransferStore } from "@/store/transfer-store";

export default function TransferManager() {
  const items = useTransferStore((s) => s.items);
  const open = useTransferStore((s) => s.ui.open);
  const setOpen = useTransferStore((s) => s.ui.setOpen);
  return (
    <Drawer open={open} onOpenChange={setOpen}>
      <DrawerContent side="right">
        <DrawerHeader>
          <DrawerTitle>Transfers</DrawerTitle>
        </DrawerHeader>
        <div className="max-h-[calc(100vh-8rem)] overflow-auto p-2 text-xs">
          {Object.keys(items).length === 0 ? (
            <div className="text-muted-foreground p-3">No active transfers</div>
          ) : (
            <ul className="space-y-2">
              {Object.values(items).map((t) => (
                <li
                  key={t.id}
                  className="flex items-center justify-between gap-2 rounded border p-2"
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">
                      {t.key || t.id}
                    </div>
                    <div className="text-muted-foreground text-[10px]">
                      {t.type} · {t.bytesDone}/{t.bytesTotal ?? "?"} · {t.state}
                    </div>
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
