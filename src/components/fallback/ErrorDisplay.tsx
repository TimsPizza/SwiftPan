"use client";

import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { AppError } from "@/lib/api/errors";
import { ExclamationTriangleIcon } from "@radix-ui/react-icons";

interface ErrorDisplayProps {
  error: AppError;
  onRetry?: () => void;
}

export const ErrorDisplay = ({ error, onRetry }: ErrorDisplayProps) => {
  return (
    <Card className="mx-auto max-w-md">
      <CardHeader className="text-center">
        <div className="mx-auto mb-4 w-fit rounded-full bg-red-100 p-3 dark:bg-red-900/20">
          <ExclamationTriangleIcon
            width="32"
            height="32"
            className="text-danger"
          />
        </div>
        <CardTitle>Something went wrong</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4 text-center">
        <p className="text-muted-foreground text-sm">
          {`${error.code} - ${error.message}` ||
            "An unexpected error occurred."}
        </p>
        {onRetry && (
          <Button variant="secondary" onClick={onRetry} className="w-full">
            Try Again
          </Button>
        )}
      </CardContent>
    </Card>
  );
};
