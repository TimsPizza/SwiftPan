"use client";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { formatBytes, truncateFilename } from "@/lib/utils";
import { UploadItem } from "@/store/upload-store";
import { motion } from "framer-motion";
import {
  AlertCircle,
  CheckCircle,
  Clock,
  Pause,
  Play,
  RotateCcw,
  Upload,
  X,
} from "lucide-react";

interface UploadProgressPanelProps {
  uploads: UploadItem[];
  onPause?: (sessionId: string) => void;
  onResume?: (sessionId: string) => void;
  onCancel?: (sessionId: string) => void;
  onRetry?: (sessionId: string) => void;
}

export const UploadProgressPanel = ({
  uploads,
  onPause,
  onResume,
  onCancel,
  onRetry,
}: UploadProgressPanelProps) => {
  if (uploads.length === 0) {
    return null;
  }

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "completed":
        return <CheckCircle className="h-4 w-4 text-green-500" />;
      case "failed":
        return <AlertCircle className="h-4 w-4 text-red-500" />;
      case "paused":
        return <Clock className="h-4 w-4 text-yellow-500" />;
      default:
        return <Upload className="h-4 w-4 text-blue-500" />;
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case "completed":
        return <Badge className="bg-green-100 text-green-800">Completed</Badge>;
      case "failed":
        return <Badge variant="destructive">Failed</Badge>;
      case "paused":
        return <Badge className="bg-yellow-100 text-yellow-800">Paused</Badge>;
      case "uploading":
        return <Badge className="bg-blue-100 text-blue-800">Uploading</Badge>;
      default:
        return <Badge variant="outline">Pending</Badge>;
    }
  };

  const totalFiles = uploads.length;
  const completedFiles = uploads.filter((u) => u.status === "success").length;
  const failedFiles = uploads.filter((u) => u.status === "error").length;
  const activeFiles = uploads.filter((u) => u.status === "uploading").length;

  return (
    <Card className="fixed right-4 bottom-4 z-50 max-h-96 w-96 overflow-hidden shadow-lg">
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">
          Upload Progress ({completedFiles}/{totalFiles})
        </CardTitle>
        <div className="flex gap-2">
          {activeFiles > 0 && (
            <Badge variant="secondary" className="bg-blue-100 text-blue-800">
              {activeFiles} uploading
            </Badge>
          )}
          {failedFiles > 0 && (
            <Badge variant="destructive">{failedFiles} failed</Badge>
          )}
        </div>
      </CardHeader>
      <CardContent className="max-h-64 space-y-3 overflow-y-auto">
        {uploads.slice(0, 5).map((upload) => (
          <motion.div
            key={upload.sessionId}
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            className="space-y-2"
          >
            <div className="flex items-center justify-between">
              <div className="flex min-w-0 flex-1 items-center gap-2">
                {getStatusIcon(upload.status)}
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">
                    {truncateFilename(upload.file.name, 25)}
                  </p>
                  <p className="text-muted-foreground text-xs">
                    {formatBytes(upload.file.size)}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-1">
                {getStatusBadge(upload.status)}
                <div className="flex gap-1">
                  {upload.status === "uploading" && onPause && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 w-6 p-0"
                      onClick={() => onPause(upload.sessionId)}
                    >
                      <Pause className="h-3 w-3" />
                    </Button>
                  )}
                  {upload.status === "paused" && onResume && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 w-6 p-0"
                      onClick={() => onResume(upload.sessionId)}
                    >
                      <Play className="h-3 w-3" />
                    </Button>
                  )}
                  {upload.status === "error" && onRetry && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 w-6 p-0"
                      onClick={() => onRetry(upload.sessionId)}
                    >
                      <RotateCcw className="h-3 w-3" />
                    </Button>
                  )}
                  {onCancel && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 w-6 p-0"
                      onClick={() => onCancel(upload.sessionId)}
                    >
                      <X className="h-3 w-3" />
                    </Button>
                  )}
                </div>
              </div>
            </div>

            {upload.status !== "success" && (
              <div className="space-y-1">
                <Progress value={upload.progress || 0} className="h-1" />
                <div className="text-muted-foreground flex justify-between text-xs">
                  <span>{upload.progress?.toFixed(1)}%</span>
                  {upload.speed && <span>{formatBytes(upload.speed)}/s</span>}
                </div>
              </div>
            )}
          </motion.div>
        ))}

        {uploads.length > 5 && (
          <p className="text-muted-foreground text-center text-xs">
            And {uploads.length - 5} more files...
          </p>
        )}
      </CardContent>
    </Card>
  );
};
