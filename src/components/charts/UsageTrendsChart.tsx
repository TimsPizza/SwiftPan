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

export interface UsageTrendsChartProps {
  points: TrendPoint[];
  showStorage?: boolean;
  showClassA?: boolean;
  showClassB?: boolean;
}

export function UsageTrendsChart({
  points,
  showStorage = true,
  showClassA = true,
  showClassB = true,
}: UsageTrendsChartProps) {
  const data = useMemo(() => {
    return (points || []).map((p) => ({
      date: p.date,
      storageMB: Math.max(
        0,
        Math.round((p.peakStorageBytes ?? 0) / 1024 / 1024),
      ),
      classA_ops: p.classACount ?? 0,
      classB_ops: p.classBCount ?? 0,
    }));
  }, [points]);

  return (
    <ResponsiveContainer width="100%" height="100%">
      <AreaChart data={data} margin={{ left: 0, right: 0, top: 10, bottom: 0 }}>
        <XAxis dataKey="date" hide />
        <YAxis hide />
        <RechartsTooltip
          formatter={(value: any, name: string) => {
            if (name === "storageMB") return [`${value} MB`, "Storage"];
            if (name === "classA_ops")
              return [`${value as number} ops`, "Class A ops"];
            if (name === "classB_ops")
              return [`${value as number} ops`, "Class B ops"];
            return [value, name];
          }}
        />
        {showStorage && (
          <Area
            type="monotone"
            dataKey="storageMB"
            stroke="var(--primary)"
            fill="var(--primary)"
            fillOpacity={0.15}
          />
        )}
        {showClassA && (
          <Area
            type="monotone"
            dataKey="classA_ops"
            stroke="var(--secondary)"
            fill="var(--secondary)"
            fillOpacity={0.15}
          />
        )}
        {showClassB && (
          <Area
            type="monotone"
            dataKey="classB_ops"
            stroke="var(--success)"
            fill="var(--success)"
            fillOpacity={0.15}
          />
        )}
      </AreaChart>
    </ResponsiveContainer>
  );
}
