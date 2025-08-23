import { settingsApi } from "@/lib/api";
import { AppError, ErrorCodes } from "@/lib/api/errors";
import { InitializeSystemRequest } from "@/lib/api/schemas";
import { useCallback, useEffect, useState } from "react";

interface SystemStatus {
  initialized: boolean;
  requiresSetup: boolean;
}

interface UseSetupReturn {
  // 状态
  status: SystemStatus | null;
  loading: boolean;
  error: AppError | null;
  submitting: boolean;

  // 方法
  checkSystemStatus: () => Promise<void>;
  initializeSystem: (data: InitializeSystemRequest) => Promise<void>;
  clearError: () => void;
}

export const useSetup = (): UseSetupReturn => {
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  const checkSystemStatus = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      // 直接使用 fetch，因为这是状态检查，可能在 API 层初始化之前
      const response = await fetch("/api/settings/status");

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new AppError(
          response.status,
          errorData.message || "Failed to check system status",
          ErrorCodes.SYSTEM_ERROR,
          errorData,
        );
      }

      const data = await response.json();
      setStatus(data.data);
    } catch (err) {
      const appError =
        err instanceof AppError
          ? err
          : new AppError(
              500,
              (err as any)?.message || "Failed to check system status",
              ErrorCodes.SYSTEM_ERROR,
              err,
            );

      setError(appError);
      throw appError;
    } finally {
      setLoading(false);
    }
  }, []);

  const initializeSystem = useCallback(
    async (data: InitializeSystemRequest) => {
      setSubmitting(true);
      setError(null);

      try {
        await settingsApi.initializeSystem(data);
      } catch (err) {
        const appError =
          err instanceof AppError
            ? err
            : new AppError(
                500,
                (err as any)?.message || "Failed to initialize system",
                ErrorCodes.SYSTEM_ERROR,
                err,
              );

        setError(appError);
        throw appError;
      } finally {
        setSubmitting(false);
      }
    },
    [],
  );

  // 初始检查系统状态
  useEffect(() => {
    checkSystemStatus();
  }, [checkSystemStatus]);

  return {
    status,
    loading,
    error,
    submitting,
    checkSystemStatus,
    initializeSystem,
    clearError,
  };
};

// 导出类型供组件使用
export type { InitializeSystemRequest, SystemStatus, UseSetupReturn };
