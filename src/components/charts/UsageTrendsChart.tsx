import type { DailyLedger } from "@/lib/api/bridge";
import { useMemo } from "react";
import * as Recharts from "recharts";

export interface UsageTrendsChartProps {
  points: DailyLedger[];
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
    return (points || []).map((p) => {
      const classASum = Object.values(p.class_a || {}).reduce(
        (acc, n) => acc + (n || 0),
        0,
      );
      const classBSum = Object.values(p.class_b || {}).reduce(
        (acc, n) => acc + (n || 0),
        0,
      );
      return {
        date: p.date,
        storageMB: Math.max(
          0,
          Math.round((p.peak_storage_bytes || 0) / 1024 / 1024),
        ),
        classA_ops: classASum,
        classB_ops: classBSum,
      };
    });
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
            if (name === "storageMB") return [`${value} MB`, "Storage"];
            if (name === "classA_ops")
              return [`${value as number} ops`, "Class A ops"];
            if (name === "classB_ops")
              return [`${value as number} ops`, "Class B ops"];
            return [value, name];
          }}
        />
        {showStorage && (
          <Recharts.Area
            type="monotone"
            dataKey="storageMB"
            stroke="var(--primary)"
            fill="var(--primary)"
            fillOpacity={0.15}
          />
        )}
        {showClassA && (
          <Recharts.Area
            type="monotone"
            dataKey="classA_ops"
            stroke="var(--secondary)"
            fill="var(--secondary)"
            fillOpacity={0.15}
          />
        )}
        {showClassB && (
          <Recharts.Area
            type="monotone"
            dataKey="classB_ops"
            stroke="var(--success)"
            fill="var(--success)"
            fillOpacity={0.15}
          />
        )}
      </Recharts.AreaChart>
    </Recharts.ResponsiveContainer>
  );
}
