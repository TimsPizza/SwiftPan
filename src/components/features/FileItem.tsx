import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import { RippleContainer } from "@/components/ui/container";
import { TableCell } from "@/components/ui/table";
import type { FileItem as TFileItem } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { formatBytes, formatRelativeTime, truncateFilename } from "@/lib/utils";
import { motion } from "framer-motion";
import {
  Check,
  DownloadIcon,
  FileIcon,
  FileText,
  Film,
  Image,
  MoreHorizontal,
  Music,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

export interface FileItemCommonProps {
  file: TFileItem;
  onSelect: (file: TFileItem) => void;
  selected?: boolean;
  onLongPress?: (point: { x: number; y: number }, file: TFileItem) => void;
  onContextMenuOpen?: (
    point: { x: number; y: number },
    file: TFileItem,
  ) => void;
  onMoreClick?: (point: { x: number; y: number }, file: TFileItem) => void;
  onDownload?: (file: TFileItem) => void;
}
const getFileIcon = (filename: string) => {
  const ext = filename.split(".").pop()?.toLowerCase();

  if (
    ["jpg", "jpeg", "png", "gif", "webp", "arw", "cr2", "svg"].includes(
      ext || "",
    )
  ) {
    return <Image className="text-blue-500" />;
  }
  if (["mp4", "avi", "mov", "wmv"].includes(ext || "")) {
    return <Film className="text-purple-500" />;
  }
  if (["mp3", "wav", "flac", "aac"].includes(ext || "")) {
    return <Music className="text-cyan-500" />;
  }
  if (
    ["pdf", "doc", "docx", "txt", "md", "json", "xml", "yaml", "yml"].includes(
      ext || "",
    )
  ) {
    return <FileText className="text-green-500" />;
  }

  return <FileIcon className="text-gray-500" />;
};

export const MobileFileItem = ({
  file,
  onSelect,
  selected,
  onLongPress,
  onContextMenuOpen,
  onMoreClick,
  onDownload,
}: FileItemCommonProps) => {
  const longPressTimerRef = useRef<number | null>(null);
  const lastPointRef = useRef<{ x: number; y: number } | null>(null);
  const [thumbUrl, setThumbUrl] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    async function loadThumb() {
      try {
        const key = (file as any).thumbnailKey as string | undefined;
        if (!key) return;
        const r = await nv.share_generate({ key, ttl_secs: 600 });
        r.match(
          (link: { url: string }) => {
            if (!alive) return;
            setThumbUrl(link.url);
          },
          () => {},
        );
      } catch {}
    }
    loadThumb();
    return () => {
      alive = false;
    };
  }, [file]);

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
    <RippleContainer
      id="file-item-container"
      className="relative flex min-w-0 flex-row items-center gap-2 rounded-2xl border py-1"
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
      <button
        id="thumbnail-container"
        className="relative ml-1 size-10 overflow-hidden rounded-full"
        onPointerDown={() => {
          onSelect(file);
        }}
        aria-pressed={!!selected}
        aria-label={selected ? "Selected" : "Select"}
      >
        <motion.div
          initial={false}
          animate={{ rotateY: selected ? 180 : 0 }}
          transition={{ type: "spring", stiffness: 260, damping: 20 }}
          className="h-full w-full [transform-style:preserve-3d]"
        >
          <div className="h-full w-full [backface-visibility:hidden]">
            {thumbUrl ? (
              <img
                src={thumbUrl}
                alt="thumbnail"
                className="h-full w-full object-cover"
                onError={(e) => {
                  (e.currentTarget as HTMLImageElement).src =
                    "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' fill='%23e5e7eb'/%3E%3C/svg%3E";
                }}
              />
            ) : (
              <div className="flex size-10 items-center justify-center">
                {getFileIcon(file.filename)}
              </div>
            )}
          </div>
          <div className="absolute inset-0 flex [transform:rotateY(180deg)] items-center justify-center rounded-full border bg-white/10 backdrop-blur-3xl [backface-visibility:hidden] dark:bg-black/10">
            <Check className="text-primary h-5 w-5" />
          </div>
        </motion.div>
      </button>
      <div
        id="file-info-container"
        className="flex min-w-0 shrink flex-col justify-start"
      >
        <p
          id="file-name"
          className="px-2 text-sm font-medium text-balance break-all"
        >
          {truncateFilename(file.filename, 11) ?? "unknown"}
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
          <DownloadIcon className="" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={(e) => onMoreClick?.({ x: e.clientX, y: e.clientY }, file)}
          aria-label="More"
          title="More"
        >
          <MoreHorizontal className="" />
        </Button>
      </div>
    </RippleContainer>
  );
};

export interface DesktopFileItemProps
  extends Omit<FileItemCommonProps, "onLongPress"> {
  index?: number;
  onDelete?: (file: TFileItem) => void;
}

export const DesktopFileItem = ({
  file,
  selected,
  onSelect,
  onDownload,
  onMoreClick,
  onDelete,
  index = 0,
}: DesktopFileItemProps) => {
  return (
    <motion.tr
      key={file.id}
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.05 }}
      className="group"
    >
      <TableCell>
        <input
          type="checkbox"
          aria-label={`Select ${file.filename}`}
          className="size-4 cursor-pointer"
          checked={!!selected}
          onChange={() => onSelect(file)}
        />
      </TableCell>
      <TableCell className="min-w-0 whitespace-normal!">
        <div className="flex min-w-0 items-center gap-3">
          {(file as any).thumbnailKey ? (
            <img
              src={`data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16'%3E%3Crect width='16' height='16' fill='%23d1d5db'/%3E%3C/svg%3E`}
              alt="thumb"
              className="h-4 w-4 rounded-sm"
            />
          ) : (
            getFileIcon(file.filename)
          )}
          <div className="flex min-w-0 flex-col">
            <p className="line-clamp-1 text-sm font-medium break-all">
              {truncateFilename(file.filename, 24)}
            </p>
            <p className="text-muted-foreground text-xs break-all">
              {truncateFilename(file.id, 18)}
            </p>
          </div>
        </div>
      </TableCell>
      <TableCell>
        <span className="text-sm font-medium">{formatBytes(file.size)}</span>
      </TableCell>
      <TableCell className="hidden lg:table-cell">
        <span className="text-muted-foreground text-sm">
          {formatRelativeTime(file.uploadedAt)}
        </span>
      </TableCell>
      <TableCell className="text-right">
        <div className="hidden items-center justify-center gap-1 md:flex">
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
            aria-label="More"
            title="More"
            onClick={(e) => onMoreClick?.({ x: e.clientX, y: e.clientY }, file)}
          >
            <MoreHorizontal className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onDelete?.(file)}
            aria-label="Delete"
            title="Delete"
          >
            {/* Reuse MoreHorizontal icon? Better to import Trash, but keep lean here. */}
            {/* Consumers can pass onDelete undefined to hide action via CSS conditions. */}
            <svg
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 24 24"
              fill="currentColor"
              className="h-4 w-4"
            >
              <path d="M9 3h6a1 1 0 0 1 1 1v1h5v2H3V5h5V4a1 1 0 0 1 1-1Zm10 6v11a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V9h14ZM9 11h2v8H9v-8Zm4 0h2v8h-2v-8Z" />
            </svg>
          </Button>
        </div>
      </TableCell>
    </motion.tr>
  );
};

export default MobileFileItem;
