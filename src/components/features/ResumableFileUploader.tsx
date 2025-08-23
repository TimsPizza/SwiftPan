"use client";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { useResumableUpload } from "@/hooks/use-resumable-upload";
import { cn, formatBytes } from "@/lib/utils";
import { UploadItem, useUploadStore } from "@/store/upload-store";
import { AnimatePresence, motion } from "framer-motion";
import {
  AlertCircle,
  CheckCircle,
  Clock,
  File,
  Pause,
  Play,
  RotateCcw,
  Upload,
  X,
} from "lucide-react";
import React, { useCallback, useRef, useState } from "react";

interface ResumableFileUploaderProps {
  onUploadComplete?: (fileId: string, fileName: string) => void;
  maxFileSize?: number;
  allowedTypes?: string[];
  variant?: "card" | "dropdown";
}

export function ResumableFileUploader({
  maxFileSize = 5 * 1024 * 1024 * 1024, // 5GB
  allowedTypes,
  variant = "card",
}: ResumableFileUploaderProps) {
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const { uploads } = useUploadStore();

  const {
    errors,
    uploadFileWithResume,
    pauseUpload,
    resumeUpload,
    removeOrCancelUpload,
    clearErrors,
    retryFailedChunks,
    autoRetryFailedUploads,
  } = useResumableUpload();

  const handleFileSelect = useCallback(
    async (files: FileList) => {
      const fileArray = Array.from(files);

      for (const file of fileArray) {
        if (file.size > maxFileSize) {
          alert(
            `File ${file.name} exceeds maximum size ${(maxFileSize / 1024 / 1024).toFixed(0)}MB`,
          );
          continue;
        }

        if (allowedTypes && !allowedTypes.includes(file.type)) {
          alert(`File type ${file.type} is not allowed for ${file.name}`);
          continue;
        }

        try {
          await uploadFileWithResume(file);
        } catch (error) {
          console.error("Upload failed:", error);
        }
      }
    },
    [maxFileSize, allowedTypes, uploadFileWithResume],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const files = e.dataTransfer.files;
      if (files.length > 0) {
        handleFileSelect(files);
      }
    },
    [handleFileSelect],
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
  }, []);

  const handleClick = () => {
    fileInputRef.current?.click();
  };

  const getStatusIcon = (upload: UploadItem) => {
    if (upload.status === "success") {
      return <CheckCircle className="h-4 w-4 text-green-500" />;
    }
    if (upload.status === "error") {
      return <AlertCircle className="h-4 w-4 text-red-500" />;
    }
    if (upload.status === "paused") {
      return <Clock className="h-4 w-4 text-yellow-500" />;
    }
    return <File className="h-4 w-4 text-blue-500" />;
  };

  const getStatusBadge = (upload: UploadItem) => {
    switch (upload.status) {
      case "success":
        return (
          <Badge variant="secondary" className="bg-green-100 text-green-800">
            Completed
          </Badge>
        );
      case "error":
        return <Badge variant="destructive">Failed</Badge>;
      case "paused":
        return (
          <Badge variant="secondary" className="bg-yellow-100 text-yellow-800">
            Paused
          </Badge>
        );
      case "uploading":
        return (
          <Badge variant="secondary" className="bg-blue-100 text-blue-800">
            Uploading
          </Badge>
        );
      default:
        return <Badge variant="outline">Pending</Badge>;
    }
  };

  const UploadArea = (
    <>
      <div
        className={cn(
          "cursor-pointer rounded-lg border-2 border-dashed p-6 text-center transition-colors",
          dragOver
            ? "border-primary bg-primary/5"
            : "border-muted-foreground/25 hover:border-primary/50",
        )}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onClick={handleClick}
      >
        <Upload className="text-muted-foreground mx-auto mb-3 h-10 w-10" />
        <div className="space-y-1.5">
          <h3 className="text-base font-semibold">
            {dragOver ? "Drop files here" : "Upload Files"}
          </h3>
          <p className="text-muted-foreground text-sm">
            Drag and drop files, or click to select
          </p>
          <p className="text-muted-foreground text-xs">
            Max size: {formatBytes(maxFileSize)}
          </p>
        </div>
      </div>
      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={(e) => {
          if (e.target.files) {
            handleFileSelect(e.target.files);
            e.target.value = "";
          }
        }}
        accept={allowedTypes?.join(",")}
      />
    </>
  );

  const ProgressList = (
    <AnimatePresence>
      {uploads.map((upload) => (
        <motion.div
          key={upload.sessionId}
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -20 }}
          className="space-y-3 rounded-lg border p-3"
        >
          <div className="flex items-center justify-between">
            <div className="flex min-w-0 flex-1 items-center gap-3">
              {getStatusIcon(upload)}
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium">
                  {upload.file.name}
                </p>
                <p className="text-muted-foreground text-xs">
                  {formatBytes(upload.file.size)}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {getStatusBadge(upload)}
              <div className="flex gap-1">
                {upload.status === "uploading" && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => pauseUpload(upload.sessionId)}
                  >
                    <Pause className="h-4 w-4" />
                  </Button>
                )}
                {upload.status === "paused" && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => resumeUpload(upload.sessionId)}
                  >
                    <Play className="h-4 w-4" />
                  </Button>
                )}
                {upload.status === "error" && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => retryFailedChunks(upload.sessionId)}
                  >
                    <RotateCcw className="h-4 w-4" />
                  </Button>
                )}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => removeOrCancelUpload(upload.sessionId)}
                >
                  <X className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
          {upload.status !== "success" && (
            <div className="space-y-2">
              <Progress value={upload.progress || 0} className="h-2" />
              <div className="text-muted-foreground flex justify-between text-xs">
                <span>{upload.progress?.toFixed(1)}% completed</span>
                {upload.speed !== 0 && (
                  <span>{formatBytes(upload.speed)}/s</span>
                )}
              </div>
            </div>
          )}
          {upload.error && (
            <Alert variant="destructive">
              <AlertCircle className="h-4 w-4" />
              <AlertDescription>{upload.error}</AlertDescription>
            </Alert>
          )}
        </motion.div>
      ))}
    </AnimatePresence>
  );

  if (variant === "dropdown") {
    return (
      <div className="max-h-[70vh] w-[380px] space-y-3 overflow-auto p-2">
        {UploadArea}
        {uploads.length > 0 && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">
                Upload Progress ({uploads.length})
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={autoRetryFailedUploads}
                disabled={!uploads.some((u) => u.status === "error")}
              >
                <RotateCcw className="mr-2 h-4 w-4" /> Retry Failed
              </Button>
            </div>
            {ProgressList}
          </div>
        )}
        {errors.length > 0 && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-destructive text-sm font-medium">
                Upload Errors
              </span>
              <Button variant="outline" size="sm" onClick={clearErrors}>
                Clear All
              </Button>
            </div>
            {errors.map((error, index) => (
              <Alert key={index} variant="destructive">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription>{error.message}</AlertDescription>
              </Alert>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardContent className="p-6">{UploadArea}</CardContent>
      </Card>

      {uploads.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center justify-between">
              <span>Upload Progress ({uploads.length} files)</span>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={autoRetryFailedUploads}
                  disabled={!uploads.some((u) => u.status === "error")}
                >
                  <RotateCcw className="mr-2 h-4 w-4" />
                  Retry Failed
                </Button>
              </div>
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">{ProgressList}</CardContent>
        </Card>
      )}

      {errors.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center justify-between">
              <span className="text-destructive">Upload Errors</span>
              <Button variant="outline" size="sm" onClick={clearErrors}>
                Clear All
              </Button>
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {errors.map((error, index) => (
              <Alert key={index} variant="destructive">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription>{error.message}</AlertDescription>
              </Alert>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
