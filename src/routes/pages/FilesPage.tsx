import GlobalError from "@/components/fallback/GlobalError";
import { FileList } from "@/components/features/FileList";
import { FileItem } from "@/lib/api/schemas";
import { queries } from "@/lib/api/tauriBridge";
import { useEffect, useState } from "react";

export default function FilesPage() {
  const [files, setFiles] = useState<FileItem[]>([]);
  const [error, setError] = useState<string | null>(null);

  const { data, isLoading, isError, refetch } =
    queries.useListAllObjects(10000);

  useEffect(() => {
    if (!data) return;
    try {
      const mapped = (data || [])
        .filter((it: any) => !it.is_prefix)
        .map((it: any) => ({
          id: it.key,
          filename: it.key.split("/").pop() || it.key,
          size: it.size ?? 0,
          mimeType: "unknown",
          uploadedAt: it.last_modified_ms ?? Date.now(),
          originalName: it.key,
        }));
      setFiles(mapped);
      setError(null);
    } catch (e: any) {
      setError(String(e?.message || e));
    }
  }, [data]);

  if (isLoading) return <div className="p-4 text-sm">Loading…</div>;
  if (isError || error) {
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
        onSecondary={isUninit ? () => void refetch() : undefined}
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
