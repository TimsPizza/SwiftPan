import GlobalError from "@/components/fallback/GlobalError";
import { FileList } from "@/components/features/FileList";
import { FileItem } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { useEffect, useState } from "react";

export default function FilesPage() {
  const [files, setFiles] = useState<FileItem[]>([]);
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
  if (error) {
    const msg = String(error || "");
    const isUninit = /credentials|not.*found|uninitialized|backend|vault/i.test(
      msg,
    );
    return (
      <GlobalError
        title={isUninit ? "SwiftPan is not initialized" : "Cannot load files"}
        description={
          isUninit
            ? "You need to configure your R2 credentials before using Files."
            : msg
        }
        primaryLabel={isUninit ? "Go to Settings" : undefined}
        onPrimary={
          isUninit ? () => (window.location.href = "/settings") : undefined
        }
        secondaryLabel={isUninit ? "Retry" : undefined}
        onSecondary={isUninit ? () => window.location.reload() : undefined}
      />
    );
  }
  if (!files.length)
    return (
      <div>
        <div className="p-4 text-sm">No files</div>
      </div>
    );
  return (
    <div className="flex min-h-0 w-full flex-col">
      <FileList files={files} />
    </div>
  );
}
