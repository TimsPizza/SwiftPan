import React from "react";
import { cn } from "@/lib/utils";

type SegmentColor = "primary" | "secondary" | "success" | "warning" | "danger";

export interface SegmentedProgressSegment {
  label: string;
  max: number; // segment capacity
  current: number; // used in this segment (will be clamped 0..max)
  color?: SegmentColor;
}

interface SegmentedProgressProps {
  segments: SegmentedProgressSegment[];
  unit?: string;
  showLegend?: boolean;
  className?: string;
}

export const SegmentedProgress = ({
  segments,
  unit = "",
  showLegend = true,
  className,
}: SegmentedProgressProps) => {
  const total = segments?.reduce((sum, s) => sum + Math.max(0, s.max), 0) ?? 0;

  const segmentBgClasses: Record<string, string> = {
    primary: "bg-primary/20",
    secondary: "bg-secondary/20",
    success: "bg-success/20",
    warning: "bg-warning/20",
    danger: "bg-danger/20",
  };
  const segmentFillClasses: Record<string, string> = {
    primary: "bg-primary",
    secondary: "bg-secondary",
    success: "bg-success",
    warning: "bg-warning",
    danger: "bg-danger",
  };

  return (
    <div className={cn("space-y-2", className)}>
      <div className="bg-muted relative h-2 w-full overflow-hidden rounded-md">
        {/* Capacity backgrounds */}
        {segments.reduce<React.ReactElement[]>((acc, seg, idx) => {
          const segTotal = Math.max(0, seg.max);
          if (total <= 0 || segTotal <= 0) return acc;
          const widthPct = (segTotal / total) * 100;
          const leftPct =
            (segments
              .slice(0, idx)
              .reduce((s, x) => s + Math.max(0, x.max), 0) /
              total) *
            100;
          acc.push(
            <div
              key={`bg-${idx}`}
              className={cn(
                "absolute top-0 h-full",
                segmentBgClasses[seg.color || "primary"],
              )}
              style={{ left: `${leftPct}%`, width: `${widthPct}%` }}
            />,
          );
          return acc;
        }, [])}

        {/* Used overlays in order */}
        {(() => {
          let remaining = segments.reduce(
            (sum, s) => sum + Math.max(0, s.current),
            0,
          );
          return segments.map((seg, idx) => {
            const segMax = Math.max(0, seg.max);
            if (total <= 0 || segMax <= 0) return null;
            const segCapacityPct = (segMax / total) * 100;
            const leftPct =
              (segments
                .slice(0, idx)
                .reduce((s, x) => s + Math.max(0, x.max), 0) /
                total) *
              100;
            const segUse = Math.max(0, Math.min(remaining, segMax));
            remaining = Math.max(0, remaining - segUse);
            const usedPct = (segUse / total) * 100;

            return (
              <div
                key={`use-${idx}`}
                className={cn(
                  "group absolute top-0 h-full transition-opacity",
                  segmentFillClasses[seg.color || "primary"],
                  usedPct === 0 && "opacity-0",
                  usedPct > 0 && "opacity-100 hover:opacity-90",
                )}
                style={{
                  left: `${leftPct}%`,
                  width: `${Math.min(usedPct, segCapacityPct)}%`,
                }}
                title={`${seg.label}`}
              >
                <div className="bg-popover text-popover-foreground pointer-events-none absolute -top-6 hidden rounded px-2 py-0.5 text-[10px] whitespace-nowrap shadow group-hover:block">
                  {seg.label}
                </div>
              </div>
            );
          });
        })()}
      </div>

      {showLegend && (
        <div className="flex flex-wrap gap-3 text-xs">
          {segments.map((seg, idx) => (
            <div key={`legend-${idx}`} className="flex items-center gap-1.5">
              <span
                className={cn(
                  "inline-block size-2.5 rounded-sm",
                  segmentFillClasses[seg.color || "primary"],
                )}
              />
              <span className="text-muted-foreground">
                {seg.label}
                {unit && ` (${unit})`}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
