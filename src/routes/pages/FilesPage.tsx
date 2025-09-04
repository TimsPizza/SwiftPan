import { FileList } from "@/components/features/FileList";
import { Button } from "@/components/ui/Button";
import { FileItem } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { useTransferStore } from "@/store/transfer-store";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { toast } from "sonner";

export default function FilesPage() {
  const [files, setFiles] = useState<FileItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const toggleTransfers = useTransferStore((s) => s.ui.toggle);
  const setTransfersOpen = useTransferStore((s) => s.ui.setOpen);

  useEffect(() => {
    (async () => {
      setLoading(true);
      setError(null);
      const r = await nv.list_all_objects(10000);
      r.match(
        (ok) => {
          // adapt to FileList schema
          const mapped = (ok || [])
            .filter((it) => !it.is_prefix)
            .map((it) => ({
              id: it.key,
              filename: it.key.split("/").pop() || it.key,
              size: it.size ?? 0,
              mimeType: "unknown",
              uploadedAt: it.last_modified_ms ?? Date.now(),
              originalName: it.key,
            }));
          setFiles(mapped);
        },
        (e) => setError(String((e as Error)?.message || e)),
      );
      setLoading(false);
    })();
  }, []);

  const handleUploadClick = async () => {
    const picked = await open({ multiple: true });
    const selected = picked ? (Array.isArray(picked) ? picked : [picked]) : [];
    if (selected.length === 0) return;
    const bucketKeys = new Set(files.map((f) => f.id));
    const storeItems = useTransferStore.getState().items;
    for (const p of selected) {
      const filePath = String(p);
      const fileName = filePath.split("/").pop() || filePath;
      const key = fileName; // flat root for now
      if (bucketKeys.has(key)) {
        toast.error(`File already exists: ${key}`);
        continue;
      }
      const dup = Object.values(storeItems).some(
        (t) =>
          t.type === "upload" &&
          t.key === key &&
          t.state !== "completed" &&
          t.state !== "failed",
      );
      if (dup) {
        toast.info(`Already uploading: ${key}`);
        continue;
      }
      const res = await nv.upload_new({
        key,
        source_path: filePath,
        part_size: 8 * 1024 * 1024,
      });
      res.match(
        () => {
          toast.success(`Upload started: ${fileName}`);
          setTransfersOpen(true);
        },
        (e) => {
          console.error(e);
          toast.error(`Upload failed to start: ${fileName}`);
        },
      );
    }
  };

  const Toolbar = (
    <div className="mb-3 flex items-center justify-end gap-2 px-4">
      <Button onClick={handleUploadClick} variant="default" size="sm">
        Upload
      </Button>
      <Button onClick={toggleTransfers} variant="outline" size="sm">
        Transfers
      </Button>
    </div>
  );

  if (loading) return <div className="p-4 text-sm">Loading…</div>;
  if (error) return <div className="p-4 text-sm text-red-600">{error}</div>;
  if (!files.length)
    return (
      <div>
        {Toolbar}
        <div className="p-4 text-sm">No files</div>
      </div>
    );
  return (
    <div>
      {Toolbar}
      <FileList files={files} />
    </div>
  );
}
