import FileItem, { DesktopFileItem } from "@/components/features/FileItem";
import {
  BatchShareDialog,
  FileDetailsDialog,
  FileItemPopOverMenu,
  FileShareDialog,
} from "@/components/features/FilePopovers";

import {
  DeleteMultiFileDialog,
  DeleteSingleFileDialog,
} from "@/components/features/DeleteDialogs";
import {
  FileDownloadDeleteTooltip,
  FileMultiSelectTooltip,
} from "@/components/features/FileTooltips";
import { Button } from "@/components/ui/Button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useFileBatchAction } from "@/hooks/use-file-batch-action";
import type { FileItem as File } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/store/app-store";
import { useTransferStore } from "@/store/transfer-store";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowUpDown,
  ChevronDown,
  ChevronUp,
  FileIcon,
  UploadIcon,
} from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

interface FileListProps {
  files: File[];
}

// Desktop cell rendering is moved into DesktopFileItem within FileItem.tsx

export const FileList = ({ files }: FileListProps) => {
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [fileToDelete, setFileToDelete] = useState<File | null>(null);
  const [multiDeleteOpen, setMultiDeleteOpen] = useState(false);
  // No custom dialogs for download/share on mobile; use OS pickers
  const [filtersOpen, setFiltersOpen] = useState(false);

  // Mobile/Desktop popover state
  // context menu
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuAnchor, setMenuAnchor] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [shareOpen, setShareOpen] = useState(false);
  const [batchShareOpen, setBatchShareOpen] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [activeFile, setActiveFile] = useState<File | null>(null);

  // Filters/sort/pagination are managed by the hook
  const setTransfersOpen = useTransferStore((s) => s.ui.setOpen);

  // batch actions using hook
  const batch = useFileBatchAction(files);

  // Pagination is fully managed by the hook

  const renderSortIcon = (key: "name" | "size" | "uploadedAt" | "type") => {
    if (batch.sortBy !== key)
      return <ArrowUpDown className="h-3.5 w-3.5 opacity-50" />;
    return batch.sortOrder === "asc" ? (
      <ChevronUp className="h-3.5 w-3.5" />
    ) : (
      <ChevronDown className="h-3.5 w-3.5" />
    );
  };

  // Prefer hook's downloadOne to avoid new tab navigation
  const handleDownload = async (file: File, dest_path: string) => {
    // dest_path is final target (may be content://). We'll stage into sandbox first.
    // Resolve sandbox directory from backend.
    let sandboxDir = await nv.download_sandbox_dir().unwrapOr("");
    sandboxDir = String(sandboxDir || "");
    if (!sandboxDir) {
      toast.error("No sandbox directory available");
      return;
    }
    // Join sandboxDir and filename
    const joinPath = (b: string, n: string) => {
      const trimmed = b.endsWith("/") || b.endsWith("\\") ? b.slice(0, -1) : b;
      const useBackslash = trimmed.includes("\\");
      const sep = useBackslash ? "\\" : "/";
      return `${trimmed}${sep}${n}`;
    };
    const sandboxPath = joinPath(sandboxDir, file.filename || "download");
    try {
      const r = await nv.download_new({
        key: file.id,
        dest_path: sandboxPath,
        chunk_size: 4 * 1024 * 1024,
      });
      await r.match(
        async (id) => {
          // Track mapping for post-completion move
          useTransferStore.getState().update(String(id), {
            id: String(id),
            type: "download",
            key: file.id,
            destPath: dest_path,
            tempPath: sandboxPath,
          });
          useTransferStore.getState().ui.setOpen(true);
          toast.info("Download started");
        },
        async (e) => {
          throw new Error(String((e as any)?.message || e));
        },
      );
    } catch (e) {
      toast.error(`Failed to start download: ${file.filename}`);
    }
  };

  const handleDeleteClick = (file: File) => {
    setFileToDelete(file);
    setDeleteDialogOpen(true);
  };

  const handleUploadClick = async () => {
    const isAndroid = /Android/i.test(navigator.userAgent || "");
    if (isAndroid) {
      // Android: always use HTML file input to avoid double picker
      const input = document.createElement("input");
      input.type = "file";
      input.multiple = true;
      input.onchange = async () => {
        const filesChosen = Array.from(input.files || []);
        if (filesChosen.length === 0) return;
        const bucketKeys = new Set(files.map((f) => f.id));
        const storeItems = useTransferStore.getState().items;
        for (const f of filesChosen) {
          const key = f.name;
          if (bucketKeys.has(key)) {
            toast.error(`File already exists: ${key}`);
            continue;
          }
          const dup = Object.values(storeItems).some(
            (t) =>
              t.type === "upload" &&
              t.key === key &&
              t.state !== "completed" &&
              t.state !== "failed",
          );
          if (dup) {
            toast.info(`Already uploading: ${key}`);
            continue;
          }
          const start = await nv.upload_new_stream({
            key,
            bytes_total: f.size,
            part_size: 8 * 1024 * 1024,
          });
          await start.match(
            async (id) => {
              toast.success(`Upload started: ${key}`);
              setTransfersOpen(true);
              const CHUNK = 1024 * 1024 * 4; // 4 MiB
              for (let offset = 0; offset < f.size; offset += CHUNK) {
                const slice = f.slice(offset, Math.min(f.size, offset + CHUNK));
                const buf = new Uint8Array(await slice.arrayBuffer());
                const r = await nv.upload_stream_write(id, buf);
                if (r.isErr()) break;
              }
              await nv.upload_stream_finish(id);
            },
            async (e) => {
              console.error(e);
              toast.error(`Upload failed to start: ${key}`);
            },
          );
        }
      };
      input.click();
      return;
    }

    const picked = await open({ multiple: true });
    const entries = picked ? (Array.isArray(picked) ? picked : [picked]) : [];
    if (entries.length === 0) return;

    const selected = entries.map((e) => String(e));
    const bucketKeys = new Set(files.map((f) => f.id));
    const storeItems = useTransferStore.getState().items;
    for (const filePath of selected) {
      const fileName = filePath.split("/").pop() || filePath;
      const key = fileName;
      if (bucketKeys.has(key)) {
        toast.error(`File already exists: ${key}`);
        continue;
      }
      const dup = Object.values(storeItems).some(
        (t) =>
          t.type === "upload" &&
          t.key === key &&
          t.state !== "completed" &&
          t.state !== "failed",
      );
      if (dup) {
        toast.info(`Already uploading: ${key}`);
        continue;
      }
      const res = await nv.upload_new({
        key,
        source_path: filePath,
        part_size: 8 * 1024 * 1024,
      });
      res.match(
        () => {
          toast.success(`Upload started: ${fileName}`);
          setTransfersOpen(true);
        },
        (e) => {
          console.error(e);
          toast.error(`Upload failed to start: ${fileName}`);
        },
      );
    }
  };

  // Shared menu handlers
  const openMenuAt = (point: { x: number; y: number }, file: File) => {
    setActiveFile(file);
    setMenuAnchor(point);
    setMenuOpen(true);
  };

  const openShare = () => {
    setMenuOpen(false);
    setShareOpen(true);
  };
  const openDetails = () => {
    setMenuOpen(false);
    setDetailsOpen(true);
  };
  const openDownload = async (file: File) => {
    // Use native save dialog directly with suggested default
    const { save } = await import("@tauri-apps/plugin-dialog");
    const { useAppStore } = await import("@/store/app-store");
    const base = useAppStore.getState().defaultDownloadDir;
    // If base unset, at least suggest filename so the dialog isn't blank
    const defaultPath =
      base && base.trim().length > 0
        ? `${base.replace(/[\\/]$/, "")}/${file.filename}`
        : file.filename;
    const picked = await save({ defaultPath });
    if (!picked) return;
    const dest = String(picked);
    await handleDownload(file, dest);
  };

  if (!files || files.length === 0) {
    return (
      <Card>
        <CardHeader className="text-center">
          <FileIcon className="text-muted-foreground mx-auto mb-4 h-12 w-12" />
          <CardTitle>No files found</CardTitle>
          <CardDescription>
            Upload your first file to get started
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  return (
    <div
      id="file-list-container"
      className="flex max-h-full min-h-0 w-full min-w-0 flex-1 flex-col gap-4"
    >
      <CardHeader className="flex flex-col gap-2">
        <CardTitle className="flex w-full items-center justify-between">
          Files ({batch.totalItems}){" "}
          <div className="ml-auto">
            <Button asChild onClick={handleUploadClick} size="sm">
              <div>
                <UploadIcon className="mr-1 h-4 w-4" />
                Upload
              </div>
            </Button>
          </div>
        </CardTitle>
        <CardDescription>Manage your uploaded files</CardDescription>
      </CardHeader>
      <CardContent className="flex h-full min-h-0 w-full flex-col gap-2">
        {/* Collapsible filter controls (all screens) */}
        <Separator className="my-2" />
        <Collapsible
          className="flex flex-col gap-2"
          open={filtersOpen}
          id="filelist-filters"
        >
          <CollapsibleTrigger
            onPointerDown={() => setFiltersOpen((v) => !v)}
            className="flex w-20"
          >
            <Button variant="outline" size="sm">
              <ChevronUp
                className={cn(
                  "transition-transform duration-200",
                  !filtersOpen && "rotate-180",
                )}
              />
              Filters
            </Button>
          </CollapsibleTrigger>

          <CollapsibleContent>
            <div className="mb-4 grid grid-cols-1 gap-2 md:grid-cols-2 lg:grid-cols-3">
              <div className="flex items-center gap-2">
                <Input
                  placeholder="Search files..."
                  value={batch.search}
                  onChange={(e) => batch.setSearch(e.target.value)}
                />
              </div>
              <div className="flex items-center gap-2">
                <Select
                  value={batch.typeFilter}
                  onValueChange={(
                    v: "all" | "image" | "video" | "audio" | "doc" | "other",
                  ) => batch.setTypeFilter(v)}
                >
                  <SelectTrigger className="w-full" size="sm">
                    <SelectValue placeholder="Type" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">All Types</SelectItem>
                    <SelectItem value="image">Images</SelectItem>
                    <SelectItem value="video">Videos</SelectItem>
                    <SelectItem value="audio">Audio</SelectItem>
                    <SelectItem value="doc">Documents</SelectItem>
                    <SelectItem value="other">Other</SelectItem>
                  </SelectContent>
                </Select>
                <Select
                  value={batch.timeFilter}
                  onValueChange={(v: "any" | "24h" | "7d" | "30d") =>
                    batch.setTimeFilter(v)
                  }
                >
                  <SelectTrigger className="w-32" size="sm">
                    <SelectValue placeholder="Uploaded" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="any">Any time</SelectItem>
                    <SelectItem value="24h">Last 24h</SelectItem>
                    <SelectItem value="7d">Last 7 days</SelectItem>
                    <SelectItem value="30d">Last 30 days</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex items-center justify-end gap-2">
                <Input
                  type="text"
                  inputMode="numeric"
                  placeholder="Min size (MB)"
                  value={batch.minSizeMB}
                  onChange={(e) => batch.setMinSizeMB(e.target.value)}
                />
                <Input
                  type="text"
                  inputMode="numeric"
                  placeholder="Max size (MB)"
                  value={batch.maxSizeMB}
                  onChange={(e) => batch.setMaxSizeMB(e.target.value)}
                />
              </div>
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground text-xs">Sort</span>
                <Select
                  value={(batch.sortBy ?? "uploadedAt") as any}
                  onValueChange={(v: "name" | "size" | "uploadedAt" | "type") =>
                    batch.setSortBy(v)
                  }
                >
                  <SelectTrigger className="w-full" size="sm">
                    <SelectValue placeholder="Sort" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="name">Name</SelectItem>
                    <SelectItem value="size">Size</SelectItem>
                    <SelectItem value="uploadedAt">Uploaded</SelectItem>
                    <SelectItem value="type">Type</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-muted-foreground text-xs">Order</span>
                <Select
                  value={batch.sortOrder}
                  onValueChange={(v: "asc" | "desc") => batch.setSortOrder(v)}
                >
                  <SelectTrigger className="w-full" size="sm">
                    <SelectValue placeholder="Order" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="asc">Ascending</SelectItem>
                    <SelectItem value="desc">Descending</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </CollapsibleContent>
        </Collapsible>
        <FileMultiSelectTooltip
          selectedFiles={batch.getSelectedFiles()}
          onSelectAll={() => batch.selectAll()}
          onDeselectAll={() => batch.deselectAll()}
        />
        <Separator className="my-2" />

        {/* desktop batch actions tooltip */}
        {batch.selectedCount > 0 && (
          <div className="mb-3 hidden items-center justify-between rounded-md border p-2 md:flex">
            <div className="text-sm">
              <span className="font-medium">{batch.selectedCount}</span>
              selected
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                className="cursor-pointer text-xs"
                onClick={async () => {
                  const base = useAppStore.getState().defaultDownloadDir;
                  const picked = await open({
                    directory: true,
                    multiple: false,
                    defaultPath: base ?? undefined,
                  });
                  if (!picked) return;
                  const chosen = String(picked);
                  // Remember as default if missing
                  if (!base || base.trim().length === 0) {
                    useAppStore.getState().setDefaultDownloadDir(chosen);
                    try {
                      const s = await nv.settings_get();
                      const app = await s.unwrapOr(null as any);
                      if (app) {
                        await (
                          await nv.settings_set({
                            ...app,
                            defaultDownloadDir: chosen,
                          })
                        ).unwrapOr(undefined);
                      }
                    } catch {}
                  }
                  await batch.batchDownload(chosen);
                }}
              >
                Download
              </Button>
              <Button
                variant="destructive"
                size="sm"
                className="p-1"
                onClick={() => setMultiDeleteOpen(true)}
              >
                <span className="cursor-pointer text-xs">Delete</span>
              </Button>
              <Button variant="ghost" size="sm" onClick={batch.clearSelection}>
                Clear
              </Button>
            </div>
          </div>
        )}

        {/* Desktop table */}
        <div className="hidden w-full min-w-0 shrink md:!flex">
          <Table className="w-full min-w-0">
            <TableHeader className="m-auto">
              <TableRow>
                <TableHead className="w-10">
                  <input
                    type="checkbox"
                    aria-label="Select all"
                    className="size-4 cursor-pointer"
                    checked={batch.allVisibleSelected}
                    onChange={batch.toggleAllVisible}
                  />
                </TableHead>
                <TableHead
                  onClick={() => batch.toggleSort("name")}
                  className="cursor-pointer select-none"
                  aria-sort={
                    batch.sortBy === "name"
                      ? batch.sortOrder === "asc"
                        ? "ascending"
                        : "descending"
                      : "none"
                  }
                >
                  <div className="flex items-center gap-1">
                    File {renderSortIcon("name")}
                  </div>
                </TableHead>
                <TableHead
                  onClick={() => batch.toggleSort("size")}
                  className="m-auto cursor-pointer select-none"
                  aria-sort={
                    batch.sortBy === "size"
                      ? batch.sortOrder === "asc"
                        ? "ascending"
                        : "descending"
                      : "none"
                  }
                >
                  <div className="flex items-center gap-1">
                    Size {renderSortIcon("size")}
                  </div>
                </TableHead>
                <TableHead
                  onClick={() => batch.toggleSort("uploadedAt")}
                  className="m-auto hidden cursor-pointer select-none lg:table-cell"
                  aria-sort={
                    batch.sortBy === "uploadedAt"
                      ? batch.sortOrder === "asc"
                        ? "ascending"
                        : "descending"
                      : "none"
                  }
                >
                  <div className="flex items-center gap-1">
                    Uploaded {renderSortIcon("uploadedAt")}
                  </div>
                </TableHead>
                <TableHead className="text-center">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {batch.pageFiles.map((file, index) => (
                <DesktopFileItem
                  key={file.id}
                  index={index}
                  file={file}
                  selected={batch.selectedIds.has(file.id)}
                  onSelect={() => batch.toggleOne(file.id)}
                  onDownload={() => openDownload(file)}
                  onMoreClick={(p) => openMenuAt(p, file)}
                  onDelete={() => handleDeleteClick(file)}
                />
              ))}
            </TableBody>
          </Table>
        </div>

        {/* Mobile list */}
        <div
          id="mobile-file-container-wrapper"
          className="md:!hiddenshrink max-h-4/5 min-h-0 overflow-y-auto"
        >
          <div className="space-y-2">
            {batch.pageFiles.map((file) => (
              <FileItem
                key={file.id}
                file={file}
                selected={batch.selectedIds.has(file.id)}
                onSelect={() => batch.toggleOne(file.id)}
                onDownload={() => openDownload(file)}
                onMoreClick={(p) => openMenuAt(p, file)}
                onLongPress={(p) => openMenuAt(p, file)}
              />
            ))}
          </div>
        </div>

        {/* Pagination Controls */}
        <div className="flex items-center justify-center pt-2">
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => batch.prevPage()}
              disabled={batch.page <= 1}
            >
              {"<"}
            </Button>
            <span className="text-xs">
              Page {batch.page} of {batch.totalPages}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => batch.nextPage()}
              disabled={batch.page >= batch.totalPages}
            >
              {">"}
            </Button>
          </div>
        </div>
      </CardContent>
      {/*  delete dialog */}
      {fileToDelete && (
        <DeleteSingleFileDialog
          open={deleteDialogOpen}
          onOpenChange={setDeleteDialogOpen}
          file={fileToDelete}
          onConfirm={async () => {
            await batch.deleteSelectedOrByFileId(fileToDelete.id);
          }}
        />
      )}
      <DeleteMultiFileDialog
        open={multiDeleteOpen}
        onOpenChange={setMultiDeleteOpen}
        count={batch.selectedCount}
        onConfirm={async () => {
          await batch.deleteSelectedOrByFileId();
        }}
      />
      {/* Shared menu popover */}
      <FileItemPopOverMenu
        open={menuOpen}
        onOpenChange={setMenuOpen}
        onOperation={(op) => {
          if (!activeFile) return;
          if (op === "download") {
            setMenuOpen(false);
            openDownload(activeFile);
          } else if (op === "share") {
            openShare();
          } else if (op === "details") {
            openDetails();
          } else if (op === "delete") {
            handleDeleteClick(activeFile);
          }
        }}
        anchorPoint={menuAnchor ?? undefined}
      />
      {/* Share dialog */}
      {activeFile && (
        <FileShareDialog
          file={activeFile}
          open={shareOpen}
          onOpenChange={setShareOpen}
        />
      )}
      {/* Details dialog */}
      {activeFile && (
        <FileDetailsDialog
          file={activeFile}
          open={detailsOpen}
          onOpenChange={setDetailsOpen}
        />
      )}
      <FileDownloadDeleteTooltip
        selectedFiles={batch.getSelectedFiles()}
        onDownloadAll={async () => {
          const { useAppStore } = await import("@/store/app-store");
          const base = useAppStore.getState().defaultDownloadDir;
          const isAndroid = /Android/i.test(navigator.userAgent || "");
          let chosen: string | null = null;
          if (isAndroid) {
            const first = batch.getSelectedFiles()[0];
            if (!first) return;
            const { save } = await import("@tauri-apps/plugin-dialog");
            const defaultPath = base && base.trim().length > 0
              ? `${base.replace(/[\\\/]$/, "")}/${first.filename}`
              : first.filename;
            const picked = await save({ defaultPath });
            if (!picked) return;
            const full = String(picked);
            const idx = Math.max(full.lastIndexOf("/"), full.lastIndexOf("\\"));
            chosen = idx >= 0 ? full.slice(0, idx) : full;
          } else {
            const picked = await open({
              directory: true,
              multiple: false,
              defaultPath: base ?? undefined,
            });
            if (!picked) return;
            chosen = String(picked);
          }
          if (!chosen) return;
          if (!base || base.trim().length === 0) {
            useAppStore.getState().setDefaultDownloadDir(chosen);
            try {
              const s = await nv.settings_get();
              const app = await s.unwrapOr(null as any);
              if (app) {
                await (
                  await nv.settings_set({ ...app, defaultDownloadDir: chosen })
                ).unwrapOr(undefined);
              }
            } catch {}
          }
          await batch.batchDownload(chosen);
        }}
        onDeleteAll={() => setMultiDeleteOpen(true)}
        onShareAll={() => setBatchShareOpen(true)}
      />
      <FileMultiSelectTooltip
        selectedFiles={batch.getSelectedFiles()}
        onSelectAll={() => batch.selectAll()}
        onDeselectAll={() => batch.deselectAll()}
      />
      <BatchShareDialog
        selectedFiles={batch.getSelectedFiles()}
        open={batchShareOpen}
        onOpenChange={setBatchShareOpen}
      />
    </div>
  );
};
