import { Button } from "@/components/ui/Button";
import { FileItem } from "@/lib/api/schemas";
import { cn } from "@/lib/utils";
import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
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
  const boxRef = useRef<HTMLDivElement | null>(null);
  const [pos, setPos] = useState<{ x: number; y: number }>(() => ({
    x: 0,
    y: 0,
  }));
  const [drag, setDrag] = useState<{
    active: boolean;
    sx: number;
    sy: number;
    bx: number;
    by: number;
  }>({
    active: false,
    sx: 0,
    sy: 0,
    bx: 0,
    by: 0,
  });

  // Initialize default position near bottom-center on mount
  useEffect(() => {
    if (typeof window === "undefined") return;
    // default left ~10vw, bottom offset ~96px
    const x = Math.round(window.innerWidth * 0.1);
    const y = Math.max(8, window.innerHeight - 120);
    setPos({ x, y });
  }, []);

  const clamp = useCallback((nx: number, ny: number) => {
    const w = typeof window !== "undefined" ? window.innerWidth : 0;
    const h = typeof window !== "undefined" ? window.innerHeight : 0;
    const el = boxRef.current;
    const bw = el?.offsetWidth ?? Math.round(w * 0.8);
    const bh = el?.offsetHeight ?? 64;
    const minX = 4;
    const minY = 84; //doubled of the multi-select tooltip height
    const maxX = Math.max(minX, w - bw - 4);
    const maxY = Math.max(minY, h - bh - 4);
    return {
      x: Math.min(maxX, Math.max(minX, nx)),
      y: Math.min(maxY, Math.max(minY, ny)),
    };
  }, []);

  const onPointerDown = (e: React.PointerEvent) => {
    // Only start drag on handle area
    e.preventDefault();
    const target = e.target as HTMLElement;
    if (!target.closest("[data-drag-handle='true']")) return;
    (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
    setDrag({
      active: true,
      sx: e.clientX,
      sy: e.clientY,
      bx: pos.x,
      by: pos.y,
    });
  };
  const onPointerMove = (e: React.PointerEvent) => {
    e.preventDefault();
    if (!drag.active) return;
    const dx = e.clientX - drag.sx;
    const dy = e.clientY - drag.sy;
    const { x, y } = clamp(drag.bx + dx, drag.by + dy);
    setPos({ x, y });
  };
  const onPointerUp = (e: React.PointerEvent) => {
    if (!drag.active) return;
    (e.target as HTMLElement).releasePointerCapture?.(e.pointerId);
    setDrag((d) => ({ ...d, active: false }));
  };
  return (
    <div
      id="file-download-delete-tooltip"
      ref={boxRef}
      className={cn(
        "fixed z-50 hidden w-[80vw] rounded-full border bg-white/10 px-3 py-2 backdrop-blur-md md:hidden! dark:bg-black/10",
        drag.active ? "cursor-grabbing" : "cursor-default",
        selectedFiles.length > 0 && "block!",
      )}
      style={{ left: pos.x, top: pos.y }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
    >
      {/* drag handle */}
      <div
        data-drag-handle="true"
        className="flex cursor-grab items-center justify-center pb-1"
      >
        <div className="bg-muted-foreground/60 h-1 w-10 rounded-full" />
      </div>
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
        "fixed top-0 left-1/2 z-50 hidden w-full -translate-x-1/2 justify-between border bg-white/30 px-4 py-2 backdrop-blur-md md:hidden! dark:bg-black/30",
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
