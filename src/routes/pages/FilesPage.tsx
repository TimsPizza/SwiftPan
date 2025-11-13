import GlobalError from "@/components/fallback/GlobalError";
import { FileList } from "@/components/features/FileList";
import { queries } from "@/lib/api/tauriBridge";
import { useFilesStore } from "@/store/files-store";
import { useEffect, useState } from "react";

export default function FilesPage() {
  const [error, setError] = useState<string | null>(null);
  const setFilesStore = useFilesStore((s) => s.setFiles);

  const { data, isLoading, isError, refetch } = queries.useListAllObjects(
    10000,
    // {
    //   staleTime: 30_000, // 30 seconds
    //   gcTime: 60_000, // 1 minute
    //   refetchOnWindowFocus: false,
    //   refetchOnReconnect: true,
    // },
  );

  useEffect(() => {
    if (!data) return;
    try {
      const all = (data || [])
        .filter((it: any) => !it.is_prefix)
        .filter((it: any) => !it.key.startsWith("analytics/"));
      const thumbs = new Set<string>();
      for (const it of all) {
        const name = it.key.split("/").pop() || it.key;
        if (name.startsWith("thumbnail_") && name.endsWith(".jpg")) {
          thumbs.add(it.key);
        }
      }
      const mapped = all
        .filter((it: any) => {
          const base = it.key.split("/").pop() || it.key;
          // hide thumbnail objects from main listing
          return !(base.startsWith("thumbnail_") && base.endsWith(".jpg"));
        })
        .map((it: any) => {
          const name = it.key.split("/").pop() || it.key;
          const thumbKey = `thumbnail_${name}.jpg`;
          const hasThumb = thumbs.has(it.key.replace(name, thumbKey));
          return {
            id: it.key,
            filename: name,
            size: it.size ?? 0,
            mimeType: "unknown",
            uploadedAt: it.last_modified_ms ?? Date.now(),
            originalName: it.key,
            thumbnailKey: hasThumb ? it.key.replace(name, thumbKey) : undefined,
          } as any;
        });
      setFilesStore(mapped);
      setError(null);
    } catch (e: any) {
      setError(String(e?.message || e));
    }
  }, [data, setFilesStore]);

  if (isLoading)
    return (
      <div className="p-4">
        <div className="bg-muted mb-3 h-6 w-48 animate-pulse rounded" />
        <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="flex items-center gap-3">
              <div className="bg-muted h-10 w-10 animate-pulse rounded-md" />
              <div className="bg-muted h-4 w-64 animate-pulse rounded" />
            </div>
          ))}
        </div>
      </div>
    );
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

  return (
    <>
      <FileList />
    </>
  );
}
