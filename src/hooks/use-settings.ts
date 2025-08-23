import { quotaApi, settingsApi, SystemConfig } from "@/lib/api";
import { AppError, ErrorCodes } from "@/lib/api/errors";
import {
  QuotaConfig,
  UpdateQuotasRequest,
  UpdateR2CredentialsRequest,
} from "@/lib/api/schemas";
import { useCallback, useEffect, useState } from "react";

interface UseSettingsReturn {
  // states
  config: SystemConfig | null;
  quotaConfig: QuotaConfig | null;
  loading: boolean;
  error: AppError | null;
  submitting: boolean;

  // methods
  loadConfig: () => Promise<void>;
  updatePassword: (data: {
    currentPassword: string;
    newPassword: string;
  }) => Promise<void>;
  updateR2Credentials: (data: UpdateR2CredentialsRequest) => Promise<void>;
  updateQuotaBudget: (data: UpdateQuotasRequest) => Promise<void>;
  clearError: () => void;
  setSubmitting: (submitting: boolean) => void;
}

export const useSettings = (): UseSettingsReturn => {
  const [config, setConfig] = useState<SystemConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      // 并行加载系统配置和预算配置
      const configData = await settingsApi.getConfig();

      setConfig(configData);
    } catch (err) {
      const appError =
        err instanceof AppError
          ? err
          : new AppError(
              500,
              (err as any)?.message || "Failed to load configuration",
              ErrorCodes.SYSTEM_ERROR,
              err,
            );

      setError(appError);
    } finally {
      setLoading(false);
    }
  }, []);

  const updatePassword = useCallback(
    async (data: { currentPassword: string; newPassword: string }) => {
      setSubmitting(true);
      setError(null);

      try {
        await settingsApi.updatePassword(data);
      } catch (err) {
        const appError =
          err instanceof AppError
            ? err
            : new AppError(
                500,
                (err as any)?.message || "Failed to update password",
                ErrorCodes.SYSTEM_ERROR,
                err,
              );

        setError(appError);
      } finally {
        setSubmitting(false);
      }
    },
    [],
  );

  const updateR2Credentials = useCallback(
    async (data: {
      endpoint: string;
      accessKeyId: string;
      secretAccessKey: string;
      bucketName: string;
    }) => {
      setSubmitting(true);
      setError(null);

      try {
        await settingsApi.updateR2Credentials(data);
        // 重新加载配置以获取最新数据
        await loadConfig();
      } catch (err) {
        const appError =
          err instanceof AppError
            ? err
            : new AppError(
                500,
                (err as any)?.message || "Failed to update R2 credentials",
                ErrorCodes.SYSTEM_ERROR,
                err,
              );

        setError(appError);
        throw appError;
      } finally {
        setSubmitting(false);
      }
    },
    [loadConfig],
  );

  const updateQuotaBudget = useCallback(async (data: UpdateQuotasRequest) => {
    setSubmitting(true);
    setError(null);

    try {
      // backend currently only accepts monthlyBudget; other fields are for future expansion
      await quotaApi.updateBudgetConfig({
        monthlyBudget: data.monthlyCostLimit,
        storageLimitGB: data.storageLimitGB,
        classALimitM: data.classALimitM,
        classBLimitM: data.classBLimitM,
      });
    } catch (err) {
      const appError =
        err instanceof AppError
          ? err
          : new AppError(
              500,
              (err as any)?.message || "Failed to update budget settings",
              ErrorCodes.SYSTEM_ERROR,
              err,
            );

      setError(appError);
      throw appError;
    } finally {
      setSubmitting(false);
    }
  }, []);

  // 初始加载
  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  return {
    config,
    quotaConfig: config?.quotaConfig ?? null,
    loading,
    error,
    submitting,
    loadConfig,
    updatePassword,
    updateR2Credentials,
    updateQuotaBudget,
    clearError,
    setSubmitting,
  };
};

// 导出类型供组件使用
export type { UseSettingsReturn };
