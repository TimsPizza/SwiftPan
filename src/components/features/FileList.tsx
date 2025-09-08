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
import { useDesktopFileTransfer } from "@/hooks/use-desktop-file-transfer";
import { useFileBatchAction } from "@/hooks/use-file-batch-action";
import { useMobileFileTransfer } from "@/hooks/use-mobile-file-transfer";
import type { FileItem as File } from "@/lib/api/schemas";
import { cn } from "@/lib/utils";
import { ArrowUpDown, ChevronDown, ChevronUp, FileIcon, UploadIcon } from "lucide-react";
import { useState } from "react";

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

  // batch actions using hook (filters/selection/delete)
  const batch = useFileBatchAction(files);
  const isAndroid = /Android/i.test(navigator.userAgent || "");
  const transfers = isAndroid
    ? useMobileFileTransfer(files)
    : useDesktopFileTransfer(files);

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

  // handled by transfer hooks now

  const handleDeleteClick = (file: File) => {
    setFileToDelete(file);
    setDeleteDialogOpen(true);
  };

  const handleUploadClick = async () => {
    await transfers.pickUploads();
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
    await transfers.downloadOne(file);
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
              <span className="font-medium">{batch.selectedCount}</span>{" "}
              selected
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                className="cursor-pointer text-xs"
                onClick={async () => {
                  await transfers.downloadMany(batch.getSelectedFiles());
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
          await transfers.downloadMany(batch.getSelectedFiles());
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
