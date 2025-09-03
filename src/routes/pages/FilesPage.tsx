import { FileList } from "@/components/features/FileList";
import { nv } from "@/lib/api/tauriBridge";
import { useEffect, useState } from "react";

export default function FilesPage() {
  const [files, setFiles] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
        (e) => setError(String((e as any)?.message || e)),
      );
      setLoading(false);
    })();
  }, []);

  if (loading) return <div className="p-4 text-sm">Loading…</div>;
  if (error) return <div className="p-4 text-sm text-red-600">{error}</div>;
  if (!files.length) return <div className="p-4 text-sm">No files</div>;
  return <FileList files={files as any} />;
}
