"use client";

import { Loader2 } from "lucide-react";

interface LoadingSpinnerProps {
  size?: "small" | "medium" | "large";
  text?: string;
}

export const LoadingSpinner = ({
  size = "medium",
  text,
}: LoadingSpinnerProps) => {
  const sizeClasses = {
    small: "h-4 w-4",
    medium: "h-6 w-6",
    large: "h-8 w-8",
  };

  return (
    <div className="flex flex-col items-center justify-center gap-3 py-10">
      <Loader2 className={`${sizeClasses[size]} text-primary animate-spin`} />
      {text && <span className="text-muted-foreground text-sm">{text}</span>}
    </div>
  );
};
