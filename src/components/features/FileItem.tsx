import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import { RippleContainer } from "@/components/ui/container";
import type { FileItem as TFileItem } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { formatBytes, truncateFilename } from "@/lib/utils";
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

interface FileItemProps {
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

const FileItem = ({
  file,
  onSelect,
  selected,
  onLongPress,
  onContextMenuOpen,
  onMoreClick,
  onDownload,
}: FileItemProps) => {
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
        onPointerDown={(e) => {
          onSelect(file);
        }}
        aria-pressed={!!selected}
        aria-label={selected ? "Selected" : "Select"}
      >
        {/* Material-like ripple for touch / click */}
        <span className="pointer-events-none absolute inset-0 overflow-hidden rounded-md">
          {/* The ripple circle */}
          <span className="ripple absolute top-1/2 left-1/2 h-0 w-0 -translate-x-1/2 -translate-y-1/2 rounded-full bg-black/20 opacity-0 transition-[width,height,opacity] duration-300 ease-out" />
        </span>
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
          <div className="absolute inset-0 flex [transform:rotateY(180deg)] items-center justify-center bg-emerald-500 [backface-visibility:hidden]">
            <Check className="h-5 w-5 text-white" />
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
          {truncateFilename(file.filename, 14) ?? "unknown"}
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

export default FileItem;
