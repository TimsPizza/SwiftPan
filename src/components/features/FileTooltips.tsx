import { Button } from "@/components/ui/Button";
import { FileItem } from "@/lib/api/schemas";
import { cn } from "@/lib/utils";
import { useEffect } from "react";
interface DownloadRemoveTooltipProps {
  selectedFiles: FileItem[];
  onDownloadAll: () => void;
  onDeleteAll: () => void;
  onShareAll: () => void;
}

interface FileMultiSelectTooltipProps {
  selectedFiles: FileItem[];
  onSelectAll: () => void;
  onDeselectAll: () => void;
}

const FileDownloadDeleteTooltip = ({
  selectedFiles,
  onDownloadAll,
  onDeleteAll,
  onShareAll,
}: DownloadRemoveTooltipProps) => {
  useEffect(() => {
    console.log("selectedFiles", selectedFiles);
  }, [selectedFiles]);
  return (
    <div
      id="file-download-delete-tooltip"
      className={cn(
        "fixed bottom-4 left-1/2 z-50 hidden w-[80vw] -translate-x-1/2 rounded-full border bg-white/70 px-4 py-2 backdrop-blur-md md:hidden dark:bg-black/70",
        selectedFiles.length > 0 && "block!",
      )}
    >
      <div className="text-foreground text-center text-xs">
        {selectedFiles.length} files selected
      </div>
      <div className="flex w-full items-center justify-between gap-2">
        <Button
          variant="outline"
          size="xs"
          onClick={onDownloadAll}
          className="min-w-18 rounded-full"
        >
          Download
        </Button>
        <Button
          variant="outline"
          size="xs"
          onClick={onShareAll}
          className="min-w-18 rounded-full"
        >
          Share
        </Button>
        <Button
          variant="destructive"
          size="xs"
          onClick={onDeleteAll}
          className="min-w-18 rounded-full"
        >
          Delete
        </Button>
      </div>
    </div>
  );
};

const FileMultiSelectTooltip = ({
  selectedFiles,
  onSelectAll,
  onDeselectAll,
}: FileMultiSelectTooltipProps) => {
  return (
    <div
      id="file-multi-select-tooltip"
      className={cn(
        "fixed top-0 left-1/2 z-50 hidden w-full -translate-x-1/2 justify-between border bg-white/70 px-4 py-2 backdrop-blur-md md:hidden dark:bg-black/70",
        selectedFiles.length > 0 && "flex!",
      )}
      style={{ top: "calc(var(--resolved-safe-top, 0px) + 42px)" }}
    >
      <Button
        variant="outline"
        className="min-w-18 rounded-full"
        size="xs"
        onClick={onSelectAll}
      >
        All
      </Button>
      <Button
        variant="outline"
        className="min-w-18 rounded-full"
        size="xs"
        onClick={onDeselectAll}
      >
        Cancel
      </Button>
    </div>
  );
};

export { FileDownloadDeleteTooltip, FileMultiSelectTooltip };
