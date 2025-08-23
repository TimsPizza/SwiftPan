import { quotaApi } from "@/lib/api";
import type { GetCostTrendsResponse } from "@/lib/api/schemas";
import { useCallback, useEffect, useState } from "react";

export type TrendPoint = GetCostTrendsResponse["trends"][number];

interface UseTrendsBaseReturn {
  trends: TrendPoint[];
  loading: boolean;
  error: Error | null;
  days: number;
  setDays: (d: number) => void;
  refetch: (d?: number) => Promise<void>;
}

export const useTrendsData = (initialDays: number = 7): UseTrendsBaseReturn => {
  const [days, setDays] = useState<number>(initialDays);
  const [trends, setTrends] = useState<TrendPoint[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<Error | null>(null);

  const fetchTrends = useCallback(async (d: number) => {
    setLoading(true);
    setError(null);
    try {
      const endTimeMs = Date.now();
      const startTimeMs = endTimeMs - d * 24 * 60 * 60 * 1000;
      const res = await quotaApi.getCostTrends({
        startms: startTimeMs,
        endms: endTimeMs,
      });
      setTrends(res.trends || []);
    } catch (e: any) {
      setError(e instanceof Error ? e : new Error(String(e?.message || e)));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchTrends(days);
  }, [days, fetchTrends]);

  const refetch = useCallback(
    async (d?: number) => {
      await fetchTrends(d ?? days);
    },
    [days, fetchTrends],
  );

  return { trends, loading, error, days, setDays, refetch };
};

// Thin specializations for clearer semantics
export const useUsageTrends = (initialDays?: number) =>
  useTrendsData(initialDays);
export const useTrafficTrends = (initialDays?: number) =>
  useTrendsData(initialDays);
export const useCostTrends = (initialDays?: number) =>
  useTrendsData(initialDays);
