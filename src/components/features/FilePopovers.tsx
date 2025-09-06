import { Badge } from "@/components/ui/badge";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { FileItem as TFileItem } from "@/lib/api/schemas";
import { formatBytes, formatRelativeTime, truncateFilename } from "@/lib/utils";

export interface FileItemPopOverMenuProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  onOperation: (
    operation: "download" | "share" | "details" | "delete" | "downloadPrompt",
  ) => void;
  trigger?: React.ReactNode;
  anchorPoint?: { x: number; y: number };
  contentClassName?: string;
}

export interface FileSharePopOverProps {
  file: TFileItem;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  centerOnMobile?: boolean;
}

export interface FileDetailsPopOverProps {
  file: TFileItem;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  centerOnMobile?: boolean;
}

export const FileItemPopOverMenu = ({
  open,
  onOpenChange,
  onOperation,
  trigger,
  anchorPoint,
  contentClassName,
}: FileItemPopOverMenuProps) => {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      {trigger ? <PopoverTrigger asChild>{trigger}</PopoverTrigger> : null}
      {anchorPoint ? (
        <PopoverAnchor
          style={{
            position: "fixed",
            left: anchorPoint.x,
            top: anchorPoint.y,
            width: 1,
            height: 1,
          }}
        />
      ) : (
        <PopoverAnchor />
      )}
      <PopoverContent
        align="start"
        side="bottom"
        sideOffset={0}
        style={
          anchorPoint
            ? { position: "fixed", left: anchorPoint.x, top: anchorPoint.y }
            : undefined
        }
        className={contentClassName}
      >
        <div
          id="file-item-popover-menu-container"
          className="flex h-36 w-14 flex-col gap-2"
        >
          <span onClick={() => onOperation("download")}>Download</span>
          <span onClick={() => onOperation("downloadPrompt")}>
            Download to...
          </span>
          <span onClick={() => onOperation("share")}>Share</span>
          <span onClick={() => onOperation("details")}>Details</span>
          <span onClick={() => onOperation("delete")}>Delete</span>
        </div>
      </PopoverContent>
    </Popover>
  );
};

export function FileDownloadPromptPopover({
  open,
  onOpenChange,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (destPath: string) => void;
}) {
  const [dest, setDest] = (globalThis as any).React?.useState("") ?? [
    "",
    () => {},
  ];
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverAnchor />
      <PopoverContent
        align="center"
        sideOffset={0}
        className="w-[min(90vw,22rem)]"
      >
        <div className="flex flex-col gap-2">
          <div className="text-sm font-medium">Download to</div>
          <input
            className="border-input bg-background text-foreground placeholder:text-muted-foreground ring-offset-background focus-visible:ring-ring flex h-9 w-full rounded-md border px-3 py-1 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium focus-visible:ring-1 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
            placeholder="/path/to/save"
            value={dest}
            onChange={(e) => setDest((e as any).target.value)}
          />
          <div className="flex items-center justify-end gap-2">
            <button
              className="rounded px-2 py-1 text-xs"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </button>
            <button
              className="bg-primary text-primary-foreground hover:bg-primary/90 rounded px-2 py-1 text-xs"
              onClick={() => {
                if (dest) onConfirm(dest);
              }}
            >
              Start
            </button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}

export const FileSharePopOver = ({
  file,
  open,
  onOpenChange,
  centerOnMobile,
}: FileSharePopOverProps) => {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverAnchor />
      <PopoverContent
        align="center"
        sideOffset={0}
        className={
          centerOnMobile
            ? "fixed top-1/2 left-1/2 z-50 w-[min(90vw,22rem)] -translate-x-1/2 -translate-y-1/2 md:relative md:top-auto md:left-auto md:w-72 md:translate-x-0 md:translate-y-0"
            : undefined
        }
      >
        <div
          id="file-share-container"
          className="flex h-36 w-14 flex-col gap-2"
        >
          <div
            id="file-share-container-header"
            className="mx-4 flex w-full items-center justify-center"
          >
            <h3>{`Create Share For ${truncateFilename(file.filename, 12) ?? "unknown"}`}</h3>
          </div>
          <div className="flex flex-col gap-2">
            <span>Expiration in:</span>
            <Select>
              <SelectTrigger>
                <SelectValue placeholder="Select expiration" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="1">1 hour</SelectItem>
                <SelectItem value="2">2 hours</SelectItem>
                <SelectItem value="3">3 hours</SelectItem>
                <SelectItem value="4">4 hours</SelectItem>
                <SelectItem value="5">5 hours</SelectItem>
                <SelectItem value="6">6 hours</SelectItem>
                <SelectItem value="7">7 hours</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <p>{`Warning: Share link cannot be tracked by the system, unexpected file download cost might be incurred.`}</p>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
};

export const FileDetailsPopOver = ({
  file,
  open,
  onOpenChange,
  centerOnMobile,
}: FileDetailsPopOverProps) => {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverAnchor />
      <PopoverContent
        align="center"
        sideOffset={0}
        className={
          centerOnMobile
            ? "fixed top-1/2 left-1/2 z-50 w-[min(90vw,22rem)] -translate-x-1/2 -translate-y-1/2 md:relative md:top-auto md:left-auto md:w-72 md:translate-x-0 md:translate-y-0"
            : undefined
        }
      >
        <div className="flex h-36 w-14 flex-col gap-2">
          <div id="file-created-at">
            <Badge>{formatRelativeTime(file.uploadedAt)}</Badge>
          </div>
          <div id="file-size">
            <Badge>{formatBytes(file.size)}</Badge>
          </div>
          <div id="file-type">
            <Badge>{file.mimeType}</Badge>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
};
