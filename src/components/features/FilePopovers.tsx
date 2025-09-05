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
  onOperation: (operation: "download" | "share" | "details" | "delete") => void;
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
        side="right"
        sideOffset={8}
        className={contentClassName}
      >
        <div
          id="file-item-popover-menu-container"
          className="flex h-36 w-14 flex-col gap-2"
        >
          <span onClick={() => onOperation("download")}>Download</span>
          <span onClick={() => onOperation("share")}>Share</span>
          <span onClick={() => onOperation("details")}>Details</span>
          <span onClick={() => onOperation("delete")}>Delete</span>
        </div>
      </PopoverContent>
    </Popover>
  );
};

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
