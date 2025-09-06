import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
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
import { Separator } from "@/components/ui/separator";
import type { FileItem as TFileItem } from "@/lib/api/schemas";
import { formatBytes, formatRelativeTime, truncateFilename } from "@/lib/utils";
import { save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { toast } from "sonner";

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

export interface FileShareDialogProps {
  file: TFileItem;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  centerOnMobile?: boolean;
}

export interface FileDetailsDialogProps {
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
        className="flex w-28 flex-col gap-1 px-2 py-1"
      >
        <div
          id="file-item-popover-menu-container"
          className="flex w-full flex-col gap-1 text-sm"
        >
          <div className="hover:bg-accent w-full cursor-pointer rounded-md p-1">
            <span onClick={() => onOperation("downloadPrompt")}>Download</span>{" "}
          </div>
          <div className="hover:bg-accent w-full cursor-pointer rounded-md p-1">
            <span onClick={() => onOperation("share")}>Share</span>
          </div>
          <div className="hover:bg-accent w-full cursor-pointer rounded-md p-1">
            <span onClick={() => onOperation("details")}>Details</span>
          </div>
          <div className="hover:bg-accent w-full cursor-pointer rounded-md p-1">
            <span onClick={() => onOperation("delete")}>Delete</span>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
};

export function FileDownloadDialog({
  open,
  onOpenChange,
  onConfirm,
  suggestedName,
  defaultDir,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (destPath: string) => void;
  suggestedName?: string;
  defaultDir?: string | null;
}) {
  const chooseAndConfirm = async () => {
    try {
      const path = await save({
        defaultPath: suggestedName
          ? `${defaultDir ?? ""}/${suggestedName}`
          : undefined,
      });
      if (path) {
        onConfirm(String(path));
      }
    } catch (e) {
      // swallow
    }
  };
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(90vw,26rem)]">
        <DialogHeader>
          <DialogTitle>Download</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <p className="text-muted-foreground text-sm">
            Choose a destination file to save.
          </p>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button onClick={chooseAndConfirm}>Choose…</Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export const FileShareDialog = ({
  file,
  open,
  onOpenChange,
}: FileShareDialogProps) => {
  const [ttlHours, setTtlHours] = useState<string>("24");
  const [pending, setPending] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const handleCreate = async () => {
    setPending(true);
    try {
      const ttl_secs = Number(ttlHours) * 3600;
      const { nv } = await import("@/lib/api/tauriBridge");
      const r = await nv.share_generate({
        key: file.id,
        ttl_secs,
        download_filename: file.filename,
      });
      r.match(
        (v: any) => setUrl(v.url),
        (e) => console.error(e),
      );
    } finally {
      setPending(false);
    }
  };
  return (
    <Dialog
      open={open}
      onOpenChange={(v: boolean) => {
        if (!v) setUrl(null);
        onOpenChange(v);
      }}
    >
      <DialogContent className="w-[min(90vw,26rem)]">
        <DialogHeader>
          <DialogTitle>{`Share ${truncateFilename(file.filename, 20) ?? "unknown"}`}</DialogTitle>
          <Separator className="my-1" />
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-row items-center gap-2">
            <span>Expiration:</span>
            <Select value={ttlHours} onValueChange={setTtlHours}>
              <SelectTrigger>
                <SelectValue placeholder="Select expiration" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="1">1 min</SelectItem>
                <SelectItem value="1">5 min</SelectItem>
                <SelectItem value="1">15 min</SelectItem>
                <SelectItem value="1">1 hour</SelectItem>
                <SelectItem value="24">24 hours</SelectItem>
                <SelectItem value="72">3 days</SelectItem>
                <SelectItem value="168">7 days</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <p className="text-muted-foreground text-xs">
            {`Warning: share links are public-accessible and the system cannot track the cost it may incur. `}
          </p>
          <p className="text-xs text-red-400">
            {`Never share them with strangers. Short expirations are recommended.`}
          </p>
          {!url ? (
            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={pending}
              >
                Cancel
              </Button>
              <Button disabled={pending} onClick={handleCreate}>
                Create
              </Button>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              <input
                className="w-full rounded border px-2 py-1"
                readOnly
                value={url}
              />
              <div className="flex justify-end gap-2">
                <Button
                  variant="outline"
                  onClick={() => {
                    void navigator.clipboard?.writeText(url);
                    toast.success("Link copied to clipboard");
                  }}
                >
                  Copy
                </Button>
                <Button onClick={() => onOpenChange(false)}>Done</Button>
              </div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
};

export const FileDetailsDialog = ({
  file,
  open,
  onOpenChange,
}: FileDetailsDialogProps) => {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(90vw,22rem)]">
        <DialogHeader>
          <DialogTitle>Details</DialogTitle>
          <Separator className="my-1" />
        </DialogHeader>
        <div className="flex flex-col justify-center gap-2">
          <div
            id="file-created-at"
            className="flex items-center justify-between gap-1"
          >
            <Label>Uploaded</Label>
            <Badge variant="outline">
              {formatRelativeTime(file.uploadedAt)}
            </Badge>
          </div>
          <div
            id="file-size"
            className="flex items-center justify-between gap-1"
          >
            <Label>Size</Label>
            <Badge variant="outline">{formatBytes(file.size)}</Badge>
          </div>
          <div
            id="file-type"
            className="flex items-center justify-between gap-1"
          >
            <Label>Type</Label>
            <Badge variant="outline">{file.mimeType}</Badge>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
};
