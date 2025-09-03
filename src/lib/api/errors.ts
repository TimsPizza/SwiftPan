export enum ErrorCodes {
  FILE_UPLOAD_FAILED = "FILE_UPLOAD_FAILED",
}

export class AppError extends Error {
  code: ErrorCodes;
  cause?: unknown;
  status?: number;
  constructor(status: number, message: string, code: ErrorCodes = ErrorCodes.FILE_UPLOAD_FAILED, cause?: unknown) {
    super(message);
    this.name = "AppError";
    this.code = code;
    this.status = status;
    this.cause = cause;
  }
}
