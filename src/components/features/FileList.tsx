"use client";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useFileBatchAction } from "@/hooks/use-file-batch-action";
import { fileApi } from "@/lib/api";
import { File } from "@/lib/api/schemas";
import { formatBytes, formatRelativeTime, truncateFilename } from "@/lib/utils";
import { motion } from "framer-motion";
import {
  ArrowUpDown,
  ChevronDown,
  ChevronUp,
  CopyIcon,
  DownloadIcon,
  FileIcon,
  FileText,
  Film,
  Image,
  MoreHorizontal,
  Music,
  TrashIcon,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "react-query";
import { toast } from "sonner";

interface FileListProps {
  files: File[];
}

const getFileIcon = (filename: string) => {
  const ext = filename.split(".").pop()?.toLowerCase();

  if (
    ["jpg", "jpeg", "png", "gif", "webp", "arw", "cr2", "svg"].includes(
      ext || "",
    )
  ) {
    return <Image className="h-4 w-4 text-blue-500" />;
  }
  if (["mp4", "avi", "mov", "wmv"].includes(ext || "")) {
    return <Film className="h-4 w-4 text-purple-500" />;
  }
  if (["mp3", "wav", "flac", "aac"].includes(ext || "")) {
    return <Music className="h-4 w-4 text-cyan-500" />;
  }
  if (
    ["pdf", "doc", "docx", "txt", "md", "json", "xml", "yaml", "yml"].includes(
      ext || "",
    )
  ) {
    return <FileText className="h-4 w-4 text-green-500" />;
  }

  return <FileIcon className="h-4 w-4 text-gray-500" />;
};

const getFileTypeColor = (filename: string) => {
  const ext = filename.split(".").pop()?.toLowerCase();

  if (
    ["jpg", "jpeg", "png", "gif", "webp", "arw", "cr2", "svg"].includes(
      ext || "",
    )
  ) {
    return "bg-blue-100 text-blue-800";
  }
  if (["mp4", "avi", "mov", "wmv"].includes(ext || "")) {
    return "bg-purple-100 text-purple-800";
  }
  if (["mp3", "wav", "flac", "aac"].includes(ext || "")) {
    return "bg-cyan-100 text-cyan-800";
  }
  if (
    ["pdf", "doc", "docx", "txt", "md", "json", "xml", "yaml", "yml"].includes(
      ext || "",
    )
  ) {
    return "bg-green-100 text-green-800";
  }

  return "bg-gray-100 text-gray-800";
};

export const FileList = ({ files }: FileListProps) => {
  const queryClient = useQueryClient();
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [fileToDelete, setFileToDelete] = useState<File | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  // Search, sort, filter state
  const [search, setSearch] = useState("");
  const [sortBy, setSortBy] = useState<
    "name" | "size" | "uploadedAt" | "type" | null
  >(null);
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("desc");
  const [typeFilter, setTypeFilter] = useState<
    "all" | "image" | "video" | "audio" | "doc" | "other"
  >("all");
  const [timeFilter, setTimeFilter] = useState<"any" | "24h" | "7d" | "30d">(
    "any",
  );
  const [minSizeMB, setMinSizeMB] = useState<string>("");
  const [maxSizeMB, setMaxSizeMB] = useState<string>("");

  const getCategory = (
    filename: string,
    mimeType?: string,
  ): "image" | "video" | "audio" | "doc" | "other" => {
    const ext = filename.split(".").pop()?.toLowerCase();
    if (
      ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff"].includes(
        ext || "",
      )
    )
      return "image";
    if (["mp4", "avi", "mov", "wmv", "mkv", "webm"].includes(ext || ""))
      return "video";
    if (["mp3", "wav", "flac", "aac", "ogg", "m4a"].includes(ext || ""))
      return "audio";
    if (
      [
        "pdf",
        "doc",
        "docx",
        "txt",
        "md",
        "xls",
        "xlsx",
        "ppt",
        "pptx",
      ].includes(ext || "")
    )
      return "doc";
    if (mimeType?.startsWith("image/")) return "image";
    if (mimeType?.startsWith("video/")) return "video";
    if (mimeType?.startsWith("audio/")) return "audio";
    return "other";
  };

  const processedFiles = useMemo(() => {
    const q = search.trim().toLowerCase();
    const now = Date.now();
    let startTime = 0;
    if (timeFilter === "24h") startTime = now - 24 * 60 * 60 * 1000;
    else if (timeFilter === "7d") startTime = now - 7 * 24 * 60 * 60 * 1000;
    else if (timeFilter === "30d") startTime = now - 30 * 24 * 60 * 60 * 1000;

    const minBytes = minSizeMB
      ? Math.max(0, Math.floor(Number(minSizeMB) * 1024 * 1024))
      : 0;
    const maxBytes = maxSizeMB
      ? Math.max(0, Math.floor(Number(maxSizeMB) * 1024 * 1024))
      : Number.POSITIVE_INFINITY;

    const filtered = (files || []).filter((f) => {
      if (!f || f.deletedAt) return false;
      if (
        q &&
        !f.filename.toLowerCase().includes(q) &&
        !f.originalName.toLowerCase().includes(q)
      )
        return false;
      if (
        typeFilter !== "all" &&
        getCategory(f.filename, f.mimeType) !== typeFilter
      )
        return false;
      if (timeFilter !== "any" && f.uploadedAt < startTime) return false;
      if (f.size < minBytes || f.size > maxBytes) return false;
      return true;
    });

    if (!sortBy) return filtered;
    const sorted = filtered.sort((a, b) => {
      const dir = sortOrder === "asc" ? 1 : -1;
      switch (sortBy) {
        case "name":
          return dir * a.filename.localeCompare(b.filename);
        case "size":
          return dir * (a.size - b.size);
        case "uploadedAt":
          return dir * (a.uploadedAt - b.uploadedAt);
        case "type": {
          const ca = getCategory(a.filename, a.mimeType);
          const cb = getCategory(b.filename, b.mimeType);
          return dir * ca.localeCompare(cb);
        }
        default:
          return 0;
      }
    });

    return sorted;
  }, [
    files,
    search,
    typeFilter,
    timeFilter,
    minSizeMB,
    maxSizeMB,
    sortBy,
    sortOrder,
  ]);

  // Pagination (10 per page)
  const PAGE_SIZE = 10;
  const [page, setPage] = useState(1);
  const totalItems = processedFiles.length;
  const totalPages = Math.max(1, Math.ceil(totalItems / PAGE_SIZE));
  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [totalPages]);
  const startIdx = (page - 1) * PAGE_SIZE;
  const endIdx = Math.min(totalItems, startIdx + PAGE_SIZE);
  const pagedFiles = useMemo(
    () => processedFiles.slice(startIdx, startIdx + PAGE_SIZE),
    [processedFiles, startIdx],
  );

  // batch actions using hook
  const batch = useFileBatchAction(files, pagedFiles);

  const toggleSort = (key: "name" | "size" | "uploadedAt" | "type") => {
    if (sortBy !== key) {
      setSortBy(key);
      setSortOrder("asc");
    } else if (sortOrder === "asc") {
      setSortOrder("desc");
    } else {
      setSortBy(null);
    }
  };

  const renderSortIcon = (key: "name" | "size" | "uploadedAt" | "type") => {
    if (sortBy !== key)
      return <ArrowUpDown className="h-3.5 w-3.5 opacity-50" />;
    return sortOrder === "asc" ? (
      <ChevronUp className="h-3.5 w-3.5" />
    ) : (
      <ChevronDown className="h-3.5 w-3.5" />
    );
  };

  const handleCopyLink = async (fileId: string, filename: string) => {
    try {
      const downloadUrl = await fileApi.getDownloadUrl(fileId);
      const fullUrl = `${downloadUrl}`;
      navigator.clipboard.writeText(fullUrl);
      toast.success("Download link copied to clipboard!");
    } catch (error) {
      toast.error("Failed to get download link");
    }
  };

  // Prefer hook's downloadOne to avoid new tab navigation
  const handleDownload = async (fileId: string, filename: string) => {
    const file = processedFiles.find((f) => f.id === fileId);
    if (!file) {
      toast.error("File not found");
      return;
    }
    await batch.downloadOne(file);
  };

  const handleDeleteClick = (file: File) => {
    setFileToDelete(file);
    setDeleteDialogOpen(true);
  };

  const handleDeleteConfirm = async () => {
    if (!fileToDelete) return;

    setIsDeleting(true);
    try {
      await fileApi.deleteFile(fileToDelete.id);
      toast.success("File deleted successfully");
      queryClient.invalidateQueries("files");
      setDeleteDialogOpen(false);
      setFileToDelete(null);
    } catch (error) {
      toast.error("Failed to delete file");
    } finally {
      setIsDeleting(false);
    }
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
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>Files ({processedFiles.length})</CardTitle>
          <CardDescription>Manage your uploaded files</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="mb-4 grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-3">
            <div className="flex items-center gap-2">
              <Input
                placeholder="Search files..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
            <div className="flex items-center gap-2">
              <Select
                value={typeFilter}
                onValueChange={(v) => setTypeFilter(v as any)}
              >
                <SelectTrigger className="w-full">
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
                value={timeFilter}
                onValueChange={(v) => setTimeFilter(v as any)}
              >
                <SelectTrigger className="w-32">
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
            <div className="flex items-center gap-2">
              <Input
                type="number"
                inputMode="numeric"
                placeholder="Min size (MB)"
                value={minSizeMB}
                onChange={(e) => setMinSizeMB(e.target.value)}
              />
              <Input
                type="number"
                inputMode="numeric"
                placeholder="Max size (MB)"
                value={maxSizeMB}
                onChange={(e) => setMaxSizeMB(e.target.value)}
              />
            </div>
          </div>

          {batch.selectedCount > 0 && (
            <div className="mb-3 flex items-center justify-between rounded-md border p-2">
              <div className="text-sm">
                <span className="font-medium">{batch.selectedCount}</span>{" "}
                selected
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={batch.batchDownload}
                >
                  Download selected
                </Button>
                <Dialog>
                  <DialogTrigger asChild>
                    <Button variant="destructive" size="sm">
                      Delete selected
                    </Button>
                  </DialogTrigger>
                  <DialogContent>
                    <DialogHeader>
                      <DialogTitle>Delete Selected Files</DialogTitle>
                      <DialogDescription>
                        Are you sure you want to delete {batch.selectedCount}{" "}
                        selected file(s)? This action cannot be undone.
                      </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                      <DialogClose asChild>
                        <Button variant="outline">Cancel</Button>
                      </DialogClose>
                      <Button
                        variant="destructive"
                        onClick={async () => {
                          await batch.deleteSelected();
                        }}
                      >
                        Confirm Delete
                      </Button>
                    </DialogFooter>
                  </DialogContent>
                </Dialog>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={batch.clearSelection}
                >
                  Clear
                </Button>
              </div>
            </div>
          )}

          <Table>
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
                  onClick={() => toggleSort("name")}
                  className="cursor-pointer select-none"
                  aria-sort={
                    sortBy === "name"
                      ? sortOrder === "asc"
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
                  onClick={() => toggleSort("type")}
                  className="m-auto hidden cursor-pointer select-none md:table-cell"
                  aria-sort={
                    sortBy === "type"
                      ? sortOrder === "asc"
                        ? "ascending"
                        : "descending"
                      : "none"
                  }
                >
                  <div className="flex items-center gap-1">
                    Type {renderSortIcon("type")}
                  </div>
                </TableHead>
                <TableHead
                  onClick={() => toggleSort("size")}
                  className="m-auto cursor-pointer select-none"
                  aria-sort={
                    sortBy === "size"
                      ? sortOrder === "asc"
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
                  onClick={() => toggleSort("uploadedAt")}
                  className="m-auto hidden cursor-pointer select-none lg:table-cell"
                  aria-sort={
                    sortBy === "uploadedAt"
                      ? sortOrder === "asc"
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
              {pagedFiles.map((file, index) =>
                file?.deletedAt ? null : (
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
                        checked={batch.selectedIds.has(file.id)}
                        onChange={() => batch.toggleOne(file.id)}
                      />
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-3">
                        {getFileIcon(file.filename)}
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-sm font-medium">
                            {truncateFilename(file.filename, 40)}
                          </p>
                          <p className="text-muted-foreground text-xs">
                            {file.id}
                          </p>
                        </div>
                      </div>
                    </TableCell>
                    <TableCell className="hidden md:table-cell">
                      <Badge
                        variant="secondary"
                        className={getFileTypeColor(file.filename)}
                      >
                        {file.mimeType === "unknown"
                          ? "unknown"
                          : file.filename.split(".").pop()?.toUpperCase() ||
                            "FILE"}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <span className="text-sm font-medium">
                        {formatBytes(file.size)}
                      </span>
                    </TableCell>
                    <TableCell className="hidden lg:table-cell">
                      <span className="text-muted-foreground text-sm">
                        {formatRelativeTime(file.uploadedAt)}
                      </span>
                    </TableCell>
                    <TableCell className="text-right">
                      {/* Inline actions on medium and larger screens */}
                      <div className="hidden items-center justify-center gap-1 md:flex">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleDownload(file.id, file.filename)}
                          aria-label="Download"
                          title="Download"
                        >
                          <DownloadIcon className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleCopyLink(file.id, file.filename)}
                          aria-label="Copy link"
                          title="Copy link"
                        >
                          <CopyIcon className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleDeleteClick(file)}
                          aria-label="Delete"
                          title="Delete"
                        >
                          <TrashIcon className="h-4 w-4" />
                        </Button>
                      </div>

                      {/* Condensed dropdown on small screens */}
                      <div className="md:hidden">
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <Button variant="ghost" size="sm">
                              <MoreHorizontal className="h-4 w-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            <DropdownMenuItem
                              onClick={() =>
                                handleDownload(file.id, file.filename)
                              }
                            >
                              <DownloadIcon className="mr-2 h-4 w-4" />
                              Download
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              onClick={() =>
                                handleCopyLink(file.id, file.filename)
                              }
                            >
                              <CopyIcon className="mr-2 h-4 w-4" />
                              Copy Link
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              onClick={() => handleDeleteClick(file)}
                              className="text-destructive focus:text-destructive"
                            >
                              <TrashIcon className="mr-2 h-4 w-4" />
                              Delete
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>
                    </TableCell>
                  </motion.tr>
                ),
              )}
            </TableBody>
          </Table>

          {/* Pagination Controls */}
          <div className="mt-4 flex items-center justify-between">
            <div className="text-muted-foreground text-sm">
              Showing {totalItems === 0 ? 0 : startIdx + 1}–{endIdx} of{" "}
              {totalItems}
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                disabled={page <= 1}
              >
                Prev
              </Button>
              <span className="text-sm">
                Page {page} of {totalPages}
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                disabled={page >= totalPages}
              >
                Next
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Dialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete File</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete "{fileToDelete?.filename}"? This
              action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDeleteDialogOpen(false)}
              disabled={isDeleting}
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={handleDeleteConfirm}
              disabled={isDeleting}
            >
              {isDeleting ? "Deleting..." : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
};
