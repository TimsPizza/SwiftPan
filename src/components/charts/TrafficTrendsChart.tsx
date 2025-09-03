import type { TrendPoint } from "@/hooks/use-trends";
import { useMemo } from "react";
import {
  Area,
  AreaChart,
  Tooltip as RechartsTooltip,
  ResponsiveContainer,
  XAxis,
  YAxis,
} from "recharts";

export interface TrafficTrendsChartProps {
  points: TrendPoint[];
}

export function TrafficTrendsChart({ points }: TrafficTrendsChartProps) {
  const data = useMemo(() => {
    return (points || []).map((p) => ({
      date: p.date,
      uploadMB: (p.uploadBytes / 1024 / 1024).toFixed(2) ?? 0,
      downloadMB: (p.downloadBytes / 1024 / 1024).toFixed(2) ?? 0,
    }));
  }, [points]);

  return (
    <ResponsiveContainer width="100%" height="100%">
      <AreaChart data={data} margin={{ left: 0, right: 0, top: 10, bottom: 0 }}>
        <XAxis dataKey="date" hide />
        <YAxis hide />
        <RechartsTooltip
          formatter={(value: any, name: string) => {
            if (name === "uploadMB")
              return [`${value as number} MB`, "Upload MB"];
            if (name === "downloadMB")
              return [`${value as number} MB`, "Download MB"];
            return [value, name];
          }}
        />
        <Area
          type="monotone"
          dataKey="uploadMB"
          stroke="#6b7280"
          fill="#6b7280"
          fillOpacity={0.15}
        />
        <Area
          type="monotone"
          dataKey="downloadMB"
          stroke="#0ea5e9"
          fill="#0ea5e9"
          fillOpacity={0.15}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
