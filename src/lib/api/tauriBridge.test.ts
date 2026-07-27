import { mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./tauriBridge";

type IpcCall = {
  cmd: string;
  args: Record<string, unknown>;
};

function captureIpcCalls(result: unknown = undefined) {
  const calls: IpcCall[] = [];
  mockIPC((cmd, args) => {
    calls.push({
      cmd,
      args: (args ?? {}) as Record<string, unknown>,
    });
    return result;
  });
  return calls;
}

describe("Tauri bridge contract", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("keeps top-level list pagination arguments in camelCase", async () => {
    const calls = captureIpcCalls({ prefix: "raw/", items: [] });

    await api.list_objects("raw/", "next-page", 250);

    expect(calls).toEqual([
      {
        cmd: "list_objects",
        args: {
          prefix: "raw/",
          token: "next-page",
          maxKeys: 250,
        },
      },
    ]);
  });

  it("keeps upload fields nested and snake_case", async () => {
    const calls = captureIpcCalls("upload-1");
    const params = {
      key: "camera/DSC00001.ARW",
      source_path: "/photos/DSC00001.ARW",
      part_size: 8 * 1024 * 1024,
      content_type: "image/x-sony-arw",
    };

    await api.upload_new(params);

    expect(calls).toEqual([
      {
        cmd: "upload_new",
        args: { params },
      },
    ]);
  });

  it("converts a typed-array upload chunk into IPC-safe numbers", async () => {
    const calls = captureIpcCalls();

    await api.upload_stream_write(
      "upload-1",
      new Uint8Array([0, 1, 127, 128, 255]),
    );

    expect(calls).toEqual([
      {
        cmd: "upload_stream_write",
        args: {
          transferId: "upload-1",
          chunk: [0, 1, 127, 128, 255],
        },
      },
    ]);
  });

  it("keeps transfer control identifiers in camelCase", async () => {
    const calls = captureIpcCalls();

    await api.download_ctrl("download-1", "pause");

    expect(calls).toEqual([
      {
        cmd: "download_ctrl",
        args: {
          transferId: "download-1",
          action: "pause",
        },
      },
    ]);
  });

  it("preserves resume-critical download fields inside params", async () => {
    const calls = captureIpcCalls("download-1");
    const params = {
      key: "camera/DSC00001.ARW",
      dest_path: "/downloads/DSC00001.ARW",
      chunk_size: 4 * 1024 * 1024,
      expected_etag: "\"source-version-1\"",
      mime: "image/x-sony-arw",
    };

    await api.download_new(params);

    expect(calls).toEqual([
      {
        cmd: "download_new",
        args: { params },
      },
    ]);
  });

  it("preserves Android picker MIME when starting a native upload", async () => {
    const calls = captureIpcCalls("upload-android-1");
    const params = {
      key: "DSC00001.ARW",
      uri: "content://photos/1",
      part_size: 8 * 1024 * 1024,
      content_type: "image/x-sony-arw",
    };

    await api.android_upload_from_uri(params);

    expect(calls).toEqual([
      {
        cmd: "android_upload_from_uri",
        args: { params },
      },
    ]);
  });
});
