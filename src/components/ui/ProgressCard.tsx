import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import type { SegmentedProgressSegment } from "@/components/ui/segmented-progress";
import { SegmentedProgress } from "@/components/ui/segmented-progress";
import { cn } from "@/lib/utils";

interface ProgressCardProps {
  title: string;
  current?: number;
  total?: number;
  unit?: string;
  color?: "primary" | "secondary" | "success" | "warning" | "danger";
  showPercentage?: boolean;
  segments?: SegmentedProgressSegment[]; // when provided, renders a multi-segment stacked progress
}

export const ProgressCard = ({
  title,
  current: singleCurrent,
  total: singleTotal,
  unit = "",
  color = "primary",
  showPercentage = true,
  segments,
}: ProgressCardProps) => {
  // Compute aggregate when segments provided
  const totalFromSegments =
    segments?.reduce((sum, s) => sum + Math.max(0, s.max), 0) ?? 0;
  let usedFromSegments = 0;
  if (segments && segments.length > 0) {
    let remaining = segments.reduce(
      (sum, s) => sum + Math.max(0, s.current),
      0,
    );
    // Clamp consumption into segments in given order
    usedFromSegments = segments.reduce((used, s) => {
      const segMax = Math.max(0, s.max);
      const segUse = Math.max(0, Math.min(remaining, segMax));
      remaining = Math.max(0, remaining - segUse);
      return used + segUse;
    }, 0);
  }

  const effectiveTotal =
    segments && segments.length > 0 ? totalFromSegments : (singleTotal ?? 0);
  const effectiveCurrent =
    segments && segments.length > 0 ? usedFromSegments : (singleCurrent ?? 0);
  const percentage =
    effectiveTotal > 0 ? (effectiveCurrent / effectiveTotal) * 100 : 0;

  const getProgressColor = () => {
    if (percentage > 90) return "danger";
    if (percentage > 75) return "warning";
    return color;
  };

  const progressColor = getProgressColor();

  const colorClasses = {
    primary: "text-primary",
    secondary: "text-secondary",
    success: "text-success",
    warning: "text-warning",
    danger: "text-danger",
  } as const;

  const progressColorClasses = {
    primary: "[&>div]:bg-primary",
    secondary: "[&>div]:bg-secondary",
    success: "[&>div]:bg-success",
    warning: "[&>div]:bg-warning",
    danger: "[&>div]:bg-danger",
  } as const;

  return (
    <Card className="border-border/50 border-2 transition-all duration-200 hover:shadow-md">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <h3 className="text-muted-foreground text-sm font-medium">{title}</h3>
        {showPercentage && (
          <span
            className={cn("text-sm font-bold", colorClasses[progressColor])}
          >
            {percentage.toFixed(1)}%
          </span>
        )}
      </CardHeader>
      <CardContent className="space-y-4">
        {segments && segments.length > 0 ? (
          <SegmentedProgress segments={segments} unit={unit} />
        ) : (
          <>
            <Progress
              value={percentage}
              className={cn("h-2", progressColorClasses[progressColor])}
            />

            <div className="text-muted-foreground flex items-center justify-between text-sm">
              <span>
                {(singleCurrent ?? 0).toLocaleString()} {unit}
              </span>
              <span>
                of {(singleTotal ?? 0).toLocaleString()} {unit}
              </span>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
};
