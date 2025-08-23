"use client";

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

export interface CostTrendsChartProps {
  points: TrendPoint[];
}

export function CostTrendsChart({ points }: CostTrendsChartProps) {
  const data = useMemo(() => {
    return (points || []).map((p) => ({
      date: p.date,
      cost: p.cost ?? 0,
    }));
  }, [points]);

  return (
    <ResponsiveContainer width="100%" height="100%">
      <AreaChart data={data} margin={{ left: 0, right: 0, top: 10, bottom: 0 }}>
        <XAxis dataKey="date" hide />
        <YAxis hide />
        <RechartsTooltip
          formatter={(value: any, name: string) => [
            `$${Number(value).toFixed(4)}`,
            "Cost",
          ]}
        />
        <Area
          type="monotone"
          dataKey="cost"
          stroke="var(--warning)"
          fill="var(--warning)"
          fillOpacity={0.15}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
