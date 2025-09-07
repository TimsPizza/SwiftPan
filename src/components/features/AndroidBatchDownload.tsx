import { Button } from "@/components/ui/Button";
import { useFileBatchAction } from "@/hooks/use-file-batch-action";
import type { FileItem } from "@/lib/api/schemas";
import { useState } from "react";
import { toast } from "sonner";

interface AndroidBatchDownloadProps {
  files: FileItem[] | undefined;
}

// Detect if running on Android
const isAndroid = typeof window !== "undefined" && 
  /Android/i.test(window.navigator?.userAgent || "");

export function AndroidBatchDownload({ files }: AndroidBatchDownloadProps) {
  const { selectedCount, batchDownloadAndroid, getSelectedFiles } = useFileBatchAction(files);
  const [isDownloading, setIsDownloading] = useState(false);

  // Only show on Android
  if (!isAndroid) {
    return null;
  }

  const handleBatchDownload = async () => {
    if (selectedCount === 0) {
      toast.error("Please select files to download");
      return;
    }

    setIsDownloading(true);
    try {
      await batchDownloadAndroid();
    } finally {
      setIsDownloading(false);
    }
  };

  return (
    <div className="flex items-center gap-2 p-4 bg-blue-50 dark:bg-blue-950/20 rounded-lg border border-blue-200 dark:border-blue-800">
      <div className="flex-1">
        <h3 className="font-medium text-blue-900 dark:text-blue-100">
          Android Batch Download
        </h3>
        <p className="text-sm text-blue-700 dark:text-blue-300">
          {selectedCount > 0 
            ? `${selectedCount} files selected for batch download`
            : "Select files to enable batch download"
          }
        </p>
      </div>
      <Button
        onClick={handleBatchDownload}
        disabled={selectedCount === 0 || isDownloading}
        className="bg-blue-600 hover:bg-blue-700 text-white"
      >
        {isDownloading ? "Downloading..." : "Batch Download"}
      </Button>
    </div>
  );
}
