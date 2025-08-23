import { quotaApi } from "@/lib/api";
import { AppError, ErrorCodes } from "@/lib/api/errors";
import type {
  GetCostTrendsResponse,
  GetQuotaStatusResponse,
  GetQuotaUsageResponse,
} from "@/lib/api/schemas";
import { useCallback, useEffect, useState } from "react";

interface QuotaData {
  status: GetQuotaStatusResponse | null;
  usage: GetQuotaUsageResponse | null;
  trends: GetCostTrendsResponse | null;
}

interface UseQuotaDataReturn {
  data: QuotaData;
  loading: boolean;
  error: AppError | null;
  refetch: (days?: number) => Promise<void>;
  lastUpdated: number | null;

  hasData: boolean;
  isPartialData: boolean; // 部分数据加载成功
  isEmpty: boolean;
  failedOperations: string[]; // 记录哪些API调用失败了
}

export const useQuotaData = (initialDays: number = 30): UseQuotaDataReturn => {
  const [data, setData] = useState<QuotaData>({
    status: null,
    usage: null,
    trends: null,
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [failedOperations, setFailedOperations] = useState<string[]>([]);
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);

  const fetchData = useCallback(async (days: number = 30) => {
    setLoading(true);
    setError(null);
    setFailedOperations([]);

    // 计算时间戳范围（用于 trends API）
    const endTimeMs = Date.now();
    const startTimeMs = endTimeMs - days * 24 * 60 * 60 * 1000;

    // 定义所有API调用
    const apiCalls = [
      { name: "status", call: () => quotaApi.getStatus() },
      { name: "usage", call: () => quotaApi.getUsageStats() },
      {
        name: "trends",
        call: () =>
          quotaApi.getCostTrends({
            startms: startTimeMs,
            endms: endTimeMs,
          }),
      },
    ];

    try {
      // 并行执行所有API调用，但允许部分失败
      const results = await Promise.allSettled(
        apiCalls.map((api) => api.call()),
      );

      const newData: QuotaData = {
        status: null,
        usage: null,
        trends: null,
      };

      const failed: string[] = [];
      let firstError: unknown = null;

      // 处理每个结果
      results.forEach((result, index) => {
        const apiName = apiCalls[index].name;

        if (result.status === "fulfilled") {
          newData[apiName as keyof QuotaData] = result.value as any;
        } else {
          failed.push(apiName);
          if (!firstError) {
            firstError = result.reason;
          }
        }
      });

      setData(newData);
      setFailedOperations(failed);

      // 如果所有API都失败了，抛出错误
      if (failed.length === apiCalls.length) {
        const error =
          firstError instanceof AppError
            ? firstError
            : new AppError(
                500,
                (firstError as any)?.message || "All quota data sources failed",
                ErrorCodes.SYSTEM_ERROR,
                { failedOperations: failed, originalError: firstError },
              );
        throw error;
      }

      // 如果部分失败，设置错误但不抛出（允许部分数据展示）
      if (failed.length > 0) {
        const error =
          firstError instanceof AppError
            ? firstError
            : new AppError(
                500,
                `Failed to load: ${failed.join(", ")}`,
                ErrorCodes.SYSTEM_ERROR,
                { failedOperations: failed, originalError: firstError },
              );
        setError(error);
      }

      // 至少有一个成功时，更新最后刷新时间
      if (failed.length < apiCalls.length) {
        setLastUpdated(Date.now());
      }
    } catch (err) {
      // 处理意外错误或全部失败的情况
      const error =
        err instanceof AppError
          ? err
          : new AppError(
              500,
              (err as any)?.message || "Failed to load quota data",
              ErrorCodes.SYSTEM_ERROR,
              { originalError: err },
            );

      setError(error);
      setFailedOperations(apiCalls.map((api) => api.name));

      // 不重新抛出错误，让组件处理显示
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData(initialDays);
  }, [fetchData, initialDays]);

  const refetch = useCallback(
    async (days: number = 30) => {
      await fetchData(days);
    },
    [fetchData],
  );

  // 计算便捷状态
  const hasData = Object.values(data).some((value) => value !== null);
  const isPartialData =
    hasData && Object.values(data).some((value) => value === null);
  const isEmpty = !hasData;

  return {
    data,
    loading,
    error,
    refetch,
    lastUpdated,
    hasData,
    isPartialData,
    isEmpty,
    failedOperations,
  };
};

// 导出类型供组件使用
export type { QuotaData, UseQuotaDataReturn };
