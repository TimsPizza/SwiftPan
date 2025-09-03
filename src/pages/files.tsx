import { ErrorDisplay } from "@/components/fallback/ErrorDisplay";
import { LoadingSpinner } from "@/components/fallback/LoadingSpinner";
import { FileList } from "@/components/features/FileList";
import { ResumableFileUploader } from "@/components/features/ResumableFileUploader";
import { Button } from "@/components/ui/Button";
import {
  Card,
  CardContent,
  CardDescription,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import { useFiles } from "@/hooks/use-files";
import { AppError } from "@/lib/api/errors";
import { cn } from "@/lib/utils";
import { FileIcon, RefreshCw, Upload } from "lucide-react";
import { Helmet } from "react-helmet-async";

export default function FilesPage() {
  const { data: files, isLoading, error, refetch } = useFiles();

  if (isLoading) {
    return (
      <div className="container mx-auto p-6">
        <LoadingSpinner size="large" text="Loading files..." />
      </div>
    );
  }

  if (error) {
    return (
      <div className="container mx-auto p-6">
        <ErrorDisplay error={error as AppError} onRetry={() => refetch()} />
      </div>
    );
  }

  const totalFiles = files?.filter((file) => !file.deletedAt).length || 0;
  const totalSize = files?.reduce((acc, file) => acc + file.size, 0) || 0;

  return (
    <>
      <Helmet>
        <title>Files - R2Vault</title>
        <meta name="description" content="Manage your files in R2Vault" />
      </Helmet>

      <div className="container mx-auto space-y-6 p-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="flex items-center gap-3 text-3xl font-bold tracking-tight">
              <FileIcon className="text-primary h-8 w-8" />
              File Manager
            </h1>
            <p className="text-muted-foreground mt-1">
              Upload, manage and organize your files
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Button variant="outline" onClick={() => refetch()}>
              <RefreshCw
                className={cn("mr-2 h-4 w-4", isLoading ? "animate-spin" : "")}
              />
              Refresh
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="default" className="gap-2">
                  <Upload className="h-4 w-4" />
                  Upload Files
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="p-0">
                <ResumableFileUploader variant="dropdown" />
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>

        <Separator />

        {/* File List */}
        <FileList files={files || []} />

        {/* Empty State */}
        {totalFiles === 0 && (
          <Card className="py-12 text-center">
            <CardContent>
              <Upload className="text-muted-foreground mx-auto mb-4 h-12 w-12" />
              <CardTitle className="mb-2">No files uploaded yet</CardTitle>
              <CardDescription className="mb-4">
                Get started by uploading your first file to R2Vault
              </CardDescription>
            </CardContent>
          </Card>
        )}
      </div>
    </>
  );
}
