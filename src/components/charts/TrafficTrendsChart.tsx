import { useMemo } from "react";
import * as Recharts from "recharts";

export interface TrendPoint {
  date: string;
  uploadBytes: number;
  downloadBytes: number;
}

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
    <Recharts.ResponsiveContainer width="100%" height="100%">
      <Recharts.AreaChart
        data={data}
        margin={{ left: 0, right: 0, top: 10, bottom: 0 }}
      >
        <Recharts.XAxis dataKey="date" hide />
        <Recharts.YAxis hide />
        <Recharts.Tooltip
          formatter={(value: any, name: string) => {
            if (name === "uploadMB")
              return [`${value as number} MB`, "Upload MB"];
            if (name === "downloadMB")
              return [`${value as number} MB`, "Download MB"];
            return [value, name];
          }}
        />
        <Recharts.Area
          type="monotone"
          dataKey="uploadMB"
          stroke="#6b7280"
          fill="#6b7280"
          fillOpacity={0.15}
        />
        <Recharts.Area
          type="monotone"
          dataKey="downloadMB"
          stroke="#0ea5e9"
          fill="#0ea5e9"
          fillOpacity={0.15}
        />
      </Recharts.AreaChart>
    </Recharts.ResponsiveContainer>
  );
}
