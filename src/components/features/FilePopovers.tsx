import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
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
import type { FileItem, FileItem as TFileItem } from "@/lib/api/schemas";
import { api } from "@/lib/api/tauriBridge";
import { formatBytes, formatRelativeTime, truncateFilename } from "@/lib/utils";
import { useState } from "react";
import { toast } from "sonner";

export interface FileItemPopOverMenuProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  onOperation: (operation: "download" | "share" | "details" | "delete") => void;
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

export interface BatchShareDialogProps {
  selectedFiles: FileItem[];
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
            <span onClick={() => onOperation("download")}>Download</span>{" "}
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

// FileDownloadDialog removed — use Tauri save dialog inline

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
      const link = await api.share_generate({
        key: file.id,
        ttl_secs,
        download_filename: file.filename,
      });
      setUrl(link.url);
      toast.success("Share link created");
    } catch (err: any) {
      console.error(err);
      toast.error(
        `Failed to create share link: ${String(err?.message ?? err ?? "unknown error")}`,
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
                <SelectItem value="0.0167">1 min</SelectItem>
                <SelectItem value="0.0833">5 min</SelectItem>
                <SelectItem value="0.25">15 min</SelectItem>
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

export const BatchShareDialog = ({
  open,
  onOpenChange,
  selectedFiles,
}: BatchShareDialogProps) => {
  const [ttlHours, setTtlHours] = useState<string>("24");
  const [loading, setLoading] = useState(false);
  const [links, setLinks] = useState<{ key: string; url: string }[] | null>(
    null,
  );
  return (
    <Dialog
      open={open}
      onOpenChange={(v: boolean) => {
        if (!v) setLinks(null);
        onOpenChange(v);
      }}
    >
      <DialogContent className="w-[min(92vw,28rem)]">
        <DialogHeader>
          <DialogTitle>Share {selectedFiles.length} file(s)</DialogTitle>
          <DialogDescription>
            Generate public links for the selected files.
          </DialogDescription>
        </DialogHeader>
        {!links ? (
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-2">
              <Label className="min-w-24">Expiration</Label>
              <Select value={ttlHours} onValueChange={setTtlHours}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Select expiration" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">1 hour</SelectItem>
                  <SelectItem value="24">24 hours</SelectItem>
                  <SelectItem value="72">3 days</SelectItem>
                  <SelectItem value="168">7 days</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <textarea
              className="min-h-32 w-full rounded border p-2 text-xs"
              readOnly
              value={links.map((l) => `${l.key}\n${l.url}`).join("\n\n")}
            />
            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => {
                  void navigator.clipboard?.writeText(
                    links!.map((l) => `${l.key}\n${l.url}`).join("\n\n"),
                  );
                  toast.success("Links copied");
                }}
              >
                Copy
              </Button>
              <Button onClick={() => onOpenChange(false)}>Done</Button>
            </div>
          </div>
        )}
        {!links ? (
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={loading}
            >
              Cancel
            </Button>
            <Button
              onClick={async () => {
                setLoading(true);
                try {
                  const ttl_secs = Number(ttlHours) * 3600;
                  const out: { key: string; url: string }[] = [];
                  const failures: string[] = [];
                  for (const f of selectedFiles) {
                    try {
                      const link = await api.share_generate({
                        key: f.id,
                        ttl_secs,
                        download_filename: f.filename,
                      });
                      out.push({ key: f.filename, url: link.url });
                    } catch (err: any) {
                      failures.push(
                        `${f.filename}: ${String(err?.message ?? err ?? "unknown error")}`,
                      );
                    }
                  }
                  if (failures.length) {
                    toast.error(
                      `Failed to create ${failures.length} link(s).\n${failures.join("\n")}`,
                    );
                  } else {
                    toast.success("Share links created");
                  }
                  setLinks(out);
                } finally {
                  setLoading(false);
                }
              }}
              disabled={loading}
            >
              Create
            </Button>
          </DialogFooter>
        ) : null}
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
            <Label>File Name</Label>
            <Badge variant="outline">{file.filename}</Badge>
          </div>
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
