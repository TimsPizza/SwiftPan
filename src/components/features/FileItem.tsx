import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import type { FileItem as TFileItem } from "@/lib/api/schemas";
import { formatBytes, truncateFilename } from "@/lib/utils";
import { DownloadIcon, MoreHorizontal } from "lucide-react";
import { useRef } from "react";

interface FileItemProps {
  file: TFileItem;
  onSelect: (file: TFileItem) => void;
  onLongPress?: (point: { x: number; y: number }, file: TFileItem) => void;
  onContextMenuOpen?: (
    point: { x: number; y: number },
    file: TFileItem,
  ) => void;
  onMoreClick?: (point: { x: number; y: number }, file: TFileItem) => void;
  onDownload?: (file: TFileItem) => void;
}

const FileItem = ({
  file,
  onSelect,
  onLongPress,
  onContextMenuOpen,
  onMoreClick,
  onDownload,
}: FileItemProps) => {
  const longPressTimerRef = useRef<number | null>(null);
  const lastPointRef = useRef<{ x: number; y: number } | null>(null);

  const handleTouchStart = (e: React.TouchEvent<HTMLDivElement>) => {
    const t = e.touches[0];
    lastPointRef.current = { x: t.clientX, y: t.clientY };
    if (longPressTimerRef.current)
      window.clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = window.setTimeout(() => {
      if (onLongPress && lastPointRef.current)
        onLongPress(lastPointRef.current, file);
    }, 450);
  };

  const cancelLongPress = () => {
    if (longPressTimerRef.current) {
      window.clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  };

  return (
    <div
      id="file-item-container"
      className="relative flex min-w-0 flex-row items-center gap-2"
      onTouchStart={handleTouchStart}
      onTouchEnd={cancelLongPress}
      onTouchMove={cancelLongPress}
      onTouchCancel={cancelLongPress}
      onContextMenu={(e) => {
        e.preventDefault();
        const p = { x: e.clientX, y: e.clientY };
        lastPointRef.current = p;
        onContextMenuOpen?.(p, file);
      }}
    >
      <div
        id="thumbnail-container"
        className="size-10 rounded-md border"
        onClick={() => onSelect(file)}
      >
        <img src="https://placehold.co/48x48" alt="thumbnail-placeholder" />
      </div>
      <div
        id="file-info-container"
        className="flex min-w-0 shrink flex-col justify-start"
      >
        <p
          id="file-name"
          className="px-2 text-sm font-medium text-balance break-all"
        >
          {truncateFilename(file.filename) ?? "unknown"}
        </p>
        <Badge
          id="file-modified-time"
          className="text-muted-foreground scale-95 text-xs break-all"
          variant="outline"
        >
          {formatBytes(file.size)}
        </Badge>
      </div>
      <div id="operation-container" className="ml-auto flex items-center gap-1">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onDownload?.(file)}
          aria-label="Download"
          title="Download"
        >
          <DownloadIcon className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={(e) => onMoreClick?.({ x: e.clientX, y: e.clientY }, file)}
          aria-label="More"
          title="More"
        >
          <MoreHorizontal className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
};

export default FileItem;
