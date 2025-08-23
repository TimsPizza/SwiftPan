import { fileApi } from "@/lib/api";
import { AppError, ErrorCodes } from "@/lib/api/errors";
import { quickHash } from "@/lib/hash/browser";
import { useUploadStore } from "@/store/upload-store";
import { fileTypeFromBlob } from "file-type";
import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "react-query";

/**
 * 增强的上传 Hook，支持断点续传、错误恢复和并发控制
 */

interface ResumableUploadState {
  isUploading: boolean;
  isPaused: boolean;
  canResume: boolean;
  errors: Array<{
    fileId: string;
    fileName: string;
    message: string;
    code: ErrorCodes;
    retryCount?: number;
  }>;
  successCount: number;
  totalCount: number;
}

interface RetryConfig {
  maxRetries: number;
  baseDelay: number;
  maxDelay: number;
  backoffMultiplier: number;
}

interface ChunkUploadTask {
  chunkIndex: number;
  partNumber: number;
  chunk: Blob;
  retryCount: number;
  lastError?: Error;
}

interface UploadSession {
  fileId: string;
  sessionId: string;
  file: File;
  uploadedParts: Set<number>;
  totalParts: number;
  chunkSize: number;
  abortController: AbortController;
  failedChunks: Map<number, ChunkUploadTask>;
}

export const useResumableUpload = () => {
  const queryClient = useQueryClient();
  const {
    addUpload,
    updateUploadProgress,
    updateTwoStageProgress,
    setUploadStatus,
  } = useUploadStore();

  const [state, setState] = useState<ResumableUploadState>({
    isUploading: false,
    isPaused: false,
    canResume: false,
    errors: [],
    successCount: 0,
    totalCount: 0,
  });

  // 上传会话管理
  const uploadSessions = useRef<Map<string, UploadSession>>(new Map()); // sessionId -> UploadSession

  // 重试配置
  const retryConfig: RetryConfig = {
    maxRetries: 3,
    baseDelay: 1000,
    maxDelay: 10000,
    backoffMultiplier: 2,
  };

  // 最大并发数
  const MAX_CONCURRENT_CHUNKS = 5;

  // 组件卸载时清理
  useEffect(() => {
    return () => {
      // 取消所有进行中的上传
      uploadSessions.current.forEach((session) => {
        session.abortController.abort();
      });
      uploadSessions.current.clear();
    };
  }, []);

  // 计算重试延迟（指数退避）
  const calculateRetryDelay = (retryCount: number): number => {
    const delay =
      retryConfig.baseDelay *
      Math.pow(retryConfig.backoffMultiplier, retryCount);
    return Math.min(delay, retryConfig.maxDelay) + Math.random() * 1000; // 添加随机抖动
  };

  // 延迟函数
  const delay = (ms: number): Promise<void> =>
    new Promise((resolve) => setTimeout(resolve, ms));

  // 暂停上传
  const pauseUpload = useCallback(
    (sessionId: string) => {
      const session = uploadSessions.current.get(sessionId);
      if (session) {
        session.abortController.abort();
      }
      setState((prev) => ({ ...prev, isPaused: true }));
      setUploadStatus(sessionId, "paused");
    },
    [setUploadStatus],
  );

  // 恢复上传
  const resumeUpload = useCallback(
    async (sessionId: string) => {
      const session = uploadSessions.current.get(sessionId);
      if (!session) {
        throw new AppError(400, "No upload session found for this file");
      }

      setState((prev) => ({
        ...prev,
        isPaused: false,
        isUploading: true,
      }));

      try {
        // 创建新的AbortController
        session.abortController = new AbortController();

        // 从断点继续上传
        await resumeMultipartUpload(session);

        // 完成上传
        await fileApi.completeUpload(session.sessionId);
        setUploadStatus(session.sessionId, "success");

        // 清理会话
        uploadSessions.current.delete(session.sessionId);
      } catch (error) {
        if (error instanceof DOMException && error.name === "AbortError") {
          return; // 用户主动取消，不处理
        }

        const appError =
          error instanceof AppError
            ? error
            : new AppError(
                500,
                "Resume upload failed",
                ErrorCodes.FILE_UPLOAD_FAILED,
              );

        setUploadStatus(session.sessionId, "error", appError.message);

        // 记录错误但保持会话，允许后续重试
        setState((prev) => ({
          ...prev,
          errors: [
            ...prev.errors,
            {
              fileId: session.fileId,
              fileName: session.file.name,
              message: appError.message,
              code: appError.code,
            },
          ],
        }));
      }
    },
    [setUploadStatus],
  );

  // 上传单个分片（带重试）
  const uploadChunkWithRetry = async (
    session: UploadSession,
    task: ChunkUploadTask,
  ): Promise<void> => {
    const { sessionId, fileId, abortController } = session;
    const { chunk, partNumber } = task;

    for (let attempt = 0; attempt <= retryConfig.maxRetries; attempt++) {
      try {
        // 检查是否被取消
        if (abortController.signal.aborted) {
          throw new DOMException("Upload aborted", "AbortError");
        }

        // 1. 获取 presigned URL
        const { url: presignedUrl } = await fileApi.getPartPresignedUrl(
          sessionId,
          partNumber,
          chunk.size,
        );

        // 2. 直传到 R2
        const response = await fileApi.putToPresignedUrl(presignedUrl, chunk, {
          signal: abortController.signal,
          onUploadProgress: (progressEvent) => {
            // 更新该分片的进度
            const chunkProgress =
              progressEvent.loaded / (progressEvent.total || chunk.size);
            updateChunkProgress(session, task.chunkIndex, chunkProgress);
          },
        });

        // 3. 确认上传
        const etag = response.etag || "";
        if (!etag) {
          throw new Error("Missing ETag in response headers");
        }

        await fileApi.confirmPartUpload(sessionId, partNumber, {
          etag,
          size: chunk.size,
          md5: undefined,
          fileId,
        });

        // 成功：标记该分片已完成
        session.uploadedParts.add(partNumber);
        session.failedChunks.delete(partNumber);

        // 更新总体进度
        updateOverallProgress(session);
        return;
      } catch (error) {
        if (error instanceof DOMException && error.name === "AbortError") {
          throw error; // 用户取消，直接抛出
        }

        task.retryCount = attempt;
        task.lastError =
          error instanceof Error ? error : new Error(String(error));

        console.warn(
          `Chunk ${partNumber} upload attempt ${attempt + 1} failed:`,
          error,
        );

        if (attempt < retryConfig.maxRetries) {
          // 等待后重试
          const retryDelay = calculateRetryDelay(attempt);
          await delay(retryDelay);
          continue;
        }

        // 所有重试都失败了
        session.failedChunks.set(partNumber, task);
        throw new AppError(
          500,
          `Failed to upload chunk ${partNumber} after ${retryConfig.maxRetries + 1} attempts`,
          ErrorCodes.FILE_UPLOAD_FAILED,
          error,
        );
      }
    }
  };

  // 更新单个分片进度
  const updateChunkProgress = (
    session: UploadSession,
    chunkIndex: number,
    progress: number,
  ) => {
    // 这里可以实现更细粒度的进度跟踪
    // 由于复杂度考虑，暂时使用整体进度更新
    updateOverallProgress(session);
  };

  // 更新整体进度
  const updateOverallProgress = (session: UploadSession) => {
    const { sessionId, uploadedParts, totalParts, file } = session;
    const completedChunks = uploadedParts.size;
    const progress = (completedChunks / totalParts) * 100;

    updateTwoStageProgress(sessionId, progress, progress, 0);
  };

  // 并发控制的分片上传
  const resumeMultipartUpload = useCallback(async (session: UploadSession) => {
    const { file, fileId, totalParts, chunkSize, uploadedParts } = session;
    const CHUNK_SIZE = chunkSize;

    // 准备所有需要上传的分片任务
    const uploadTasks: ChunkUploadTask[] = [];

    for (let i = 0; i < totalParts; i++) {
      const partNumber = i + 1;

      // 跳过已上传的分片
      if (uploadedParts.has(partNumber)) {
        continue;
      }

      const start = i * CHUNK_SIZE;
      const end = Math.min(start + CHUNK_SIZE, file.size);
      const chunk = file.slice(start, end);

      uploadTasks.push({
        chunkIndex: i,
        partNumber,
        chunk,
        retryCount: 0,
      });
    }

    // 如果没有需要上传的分片，直接返回
    if (uploadTasks.length === 0) {
      return;
    }

    // 并发上传控制
    const semaphore = new Array(MAX_CONCURRENT_CHUNKS).fill(null);
    let taskIndex = 0;

    const uploadWorker = async (): Promise<void> => {
      while (taskIndex < uploadTasks.length) {
        const currentTaskIndex = taskIndex++;
        const task = uploadTasks[currentTaskIndex];

        try {
          await uploadChunkWithRetry(session, task);
        } catch (error) {
          // 单个分片失败不影响其他分片继续上传
          console.error(
            `Chunk ${task.partNumber} failed after all retries:`,
            error,
          );
        }
      }
    };

    // 启动并发上传工作器
    const workers = semaphore.map(() => uploadWorker());
    await Promise.allSettled(workers);

    // 检查是否有失败的分片
    if (session.failedChunks.size > 0) {
      const failedParts = Array.from(session.failedChunks.keys());
      throw new AppError(
        500,
        `Upload incomplete. Failed parts: ${failedParts.join(", ")}`,
        ErrorCodes.FILE_UPLOAD_FAILED,
      );
    }
  }, []);

  // 重试失败的分片
  const retryFailedChunks = useCallback(
    async (sessionId: string) => {
      const session = uploadSessions.current.get(sessionId);
      if (!session || session.failedChunks.size === 0) {
        return;
      }

      setState((prev) => ({ ...prev, isUploading: true }));

      try {
        // 重置失败分片的重试计数
        session.failedChunks.forEach((task) => {
          task.retryCount = 0;
        });

        // 重新上传失败的分片
        const failedTasks = Array.from(session.failedChunks.values());
        const workers = Array(
          Math.min(MAX_CONCURRENT_CHUNKS, failedTasks.length),
        )
          .fill(null)
          .map(async () => {
            let taskIndex = 0;
            while (taskIndex < failedTasks.length) {
              const task = failedTasks[taskIndex++];
              if (session.failedChunks.has(task.partNumber)) {
                await uploadChunkWithRetry(session, task);
              }
            }
          });

        await Promise.allSettled(workers);

        // 如果还有失败的分片，抛出错误
        if (session.failedChunks.size > 0) {
          throw new AppError(500, "Some chunks still failed after retry");
        }

        // 完成上传
        await fileApi.completeUpload(session.sessionId);
        setUploadStatus(session.sessionId, "success");
        uploadSessions.current.delete(session.sessionId);
      } catch (error) {
        const appError =
          error instanceof AppError
            ? error
            : new AppError(500, "Retry failed", ErrorCodes.FILE_UPLOAD_FAILED);

        setUploadStatus(session.sessionId, "error", appError.message);
      } finally {
        setState((prev) => ({ ...prev, isUploading: false }));
      }
    },
    [setUploadStatus],
  );

  // 增强的上传文件函数
  const uploadFileWithResume = useCallback(
    async (file: File) => {
      // 先准备上传，拿到 sessionId 再入 store

      try {
        const detectMimeType = async (f: File): Promise<string> => {
          if (f.type) return f.type;
          try {
            const detected = await fileTypeFromBlob(f);
            if (detected?.mime) return detected.mime;
          } catch {}
          const lowerName = f.name.toLowerCase();
          const ext = lowerName.includes(".")
            ? lowerName.substring(lowerName.lastIndexOf(".") + 1)
            : "";
          const byExt: Record<string, string> = {
            md: "text/markdown",
            markdown: "text/markdown",
            txt: "text/plain",
            csv: "text/csv",
            json: "application/json",
            xml: "application/xml",
            yaml: "text/yaml",
            yml: "text/yaml",
            svg: "image/svg+xml",
            js: "text/javascript",
            mjs: "text/javascript",
            ts: "text/plain",
            tsx: "text/plain",
            jsx: "text/javascript",
            html: "text/html",
            css: "text/css",
          };
          return byExt[ext] || "application/octet-stream";
        };

        // 1. 准备上传
        const prepData = await fileApi.prepareUpload({
          filename: file.name,
          size: file.size,
          mimeType: await detectMimeType(file),
          hash: await quickHash(file),
        });

        const fileId = prepData.fileId;
        const sessionId = prepData.sessionId!;

        // 入 store（以 sessionId 作为主键）
        addUpload({
          id: sessionId,
          sessionId,
          file,
          progress: 0,
          clientProgress: 0,
          confirmProgress: 0,
          speed: 0,
          status: "pending",
        });

        // 创建上传会话
        const abortController = new AbortController();
        const session: UploadSession = {
          fileId,
          sessionId: sessionId!,
          file,
          uploadedParts: new Set(),
          totalParts: prepData.totalParts || 1,
          chunkSize: prepData.chunkSize || file.size,
          abortController,
          failedChunks: new Map(),
        };

        uploadSessions.current.set(sessionId, session);

        setState((prev) => ({ ...prev, isUploading: true }));

        // 3. 执行上传
        if (prepData.mode === "single") {
          // 单文件上传（直传模式）
          if (prepData.uploadInfo?.url) {
            await fileApi.putToPresignedUrl(prepData.uploadInfo!.url, file, {
              signal: abortController.signal,
              onUploadProgress: (progressEvent) => {
                const progress =
                  progressEvent.loaded / (progressEvent.total || file.size);
                updateTwoStageProgress(sessionId, progress * 100, 0, 0);
              },
            });
          } else {
            throw new AppError(500, "Missing presigned URL for single upload");
          }

          // 单文件直传完成后，调用完成确认接口
          await fileApi.completeUpload(sessionId);
          updateTwoStageProgress(sessionId, 100, 100, 0);
        } else {
          // 分块上传（支持断点续传和并发）
          await resumeMultipartUpload(session);
          await fileApi.completeUpload(sessionId);
        }

        // 4. 完成上传
        setUploadStatus(sessionId, "success");
        queryClient.invalidateQueries("files");
        uploadSessions.current.delete(sessionId);

        return { fileId, fileName: file.name, sessionId };
      } catch (error) {
        if (error instanceof DOMException && error.name === "AbortError") {
          return; // 用户取消，不处理
        }

        const fallbackId =
          useUploadStore.getState().uploads.find((u) => u.file === file)?.id ||
          "";
        const appError =
          error instanceof AppError
            ? error
            : new AppError(
                500,
                "Upload failed",
                ErrorCodes.FILE_UPLOAD_FAILED,
                error,
              );

        if (fallbackId) setUploadStatus(fallbackId, "error", appError.message);

        // 记录错误
        setState((prev) => ({
          ...prev,
          errors: [
            ...prev.errors,
            {
              fileId: "",
              fileName: file.name,
              message: appError.message,
              code: appError.code,
            },
          ],
          isUploading: false,
        }));
      }
    },
    [addUpload, queryClient, setUploadStatus],
  );

  // 清除错误状态
  const clearErrors = useCallback(() => {
    setState((prev) => ({ ...prev, errors: [] }));
  }, []);

  // 取消上传或删除上传记录
  const removeOrCancelUpload = useCallback(async (sessionId: string) => {
    console.log("removeOrCancelUpload", sessionId);
    const session = uploadSessions.current.get(sessionId);
    if (session) {
      // 取消网络请求
      session.abortController.abort();

      try {
        await fileApi.cancelUpload(session.sessionId);
      } catch (error) {
        console.error("Failed to cancel upload on server:", error);
      }

      // 清除会话
      uploadSessions.current.delete(sessionId);
    } else {
      useUploadStore.setState((state) => ({
        uploads: state.uploads.filter((u) => u.id !== sessionId),
      }));
    }
  }, []);

  // 自动重试失败的上传
  const autoRetryFailedUploads = useCallback(async () => {
    const failedSessions = Array.from(uploadSessions.current.values()).filter(
      (session) => session.failedChunks.size > 0,
    );

    for (const session of failedSessions) {
      try {
        await retryFailedChunks(session.sessionId);
      } catch (error) {
        console.warn(`Auto retry failed for ${session.sessionId}:`, error);
      }
    }
  }, [retryFailedChunks]);

  // 获取暂停的上传（兼容原接口）
  const pausedUploads = Array.from(uploadSessions.current.values())
    .filter((session) => {
      // 通过store查找对应的upload状态
      const upload = useUploadStore
        .getState()
        .uploads.find((u) => u.id === session.sessionId);
      return upload?.status === "paused";
    })
    .map((session) => ({
      fileId: session.fileId,
      sessionId: session.sessionId,
      file: session.file,
      uploadedParts: Array.from(session.uploadedParts),
      totalParts: session.totalParts,
      chunkSize: session.chunkSize,
    }));

  return {
    ...state,
    uploadFileWithResume,
    pauseUpload,
    resumeUpload,
    removeOrCancelUpload,
    clearErrors,
    retryFailedChunks,
    autoRetryFailedUploads,
    // 兼容原接口
    pausedUploads,
    // 返回活跃会话信息
    activeSessions: Array.from(uploadSessions.current.values()).map(
      (session) => ({
        fileId: session.fileId,
        fileName: session.file.name,
        uploadedParts: session.uploadedParts.size,
        totalParts: session.totalParts,
        failedChunks: session.failedChunks.size,
        progress: (session.uploadedParts.size / session.totalParts) * 100,
      }),
    ),
  };
};
