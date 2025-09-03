// Minimal placeholder API for type-check only
export const fileApi = {
  async prepareUpload(_: { filename: string; size: number; mimeType: string; hash: string }) {
    return {
      fileId: crypto.randomUUID?.() || `${Date.now()}`,
      sessionId: crypto.randomUUID?.() || `${Date.now()}-s`,
      totalParts: 1,
      chunkSize: _?.size || 0,
      mode: _?.size > 0 ? "single" : "single",
      uploadInfo: { url: "about:blank" },
    } as any;
  },
  async getPartPresignedUrl(_: string, __: number, ___: number) {
    return { url: "about:blank" } as any;
  },
  async putToPresignedUrl(_url: string, _body: Blob, _opts?: any) {
    // pretend it uploaded and return an etag-like string
    return { etag: '"mock-etag"' } as any;
  },
  async confirmPartUpload(_: string, __: number, ___: any) {
    return {} as any;
  },
  async completeUpload(_: string) {
    return {} as any;
  },
  async cancelUpload(_: string) {
    return {} as any;
  },
} as const;

export const settingsService = {
  async get() { return {} as any; },
  async update(_: any) { return {} as any; },
};
