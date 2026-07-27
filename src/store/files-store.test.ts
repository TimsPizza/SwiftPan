import type { FileItem } from "@/lib/api/schemas";
import { beforeEach, describe, expect, it } from "vitest";
import { useFilesStore } from "./files-store";

function file(id: string, size: number): FileItem {
  return {
    id,
    filename: id,
    originalName: id,
    size,
    uploadedAt: 1,
  };
}

describe("files store", () => {
  beforeEach(() => {
    useFilesStore.setState({ files: [] });
  });

  it("deduplicates by object key and keeps the newest value", () => {
    useFilesStore.getState().setFiles([
      file("same.ARW", 10),
      file("other.ARW", 20),
      file("same.ARW", 30),
    ]);

    expect(useFilesStore.getState().files).toEqual([
      file("same.ARW", 30),
      file("other.ARW", 20),
    ]);
  });

  it("upserts without dropping unrelated objects", () => {
    useFilesStore.getState().setFiles([
      file("old.ARW", 10),
      file("keep.ARW", 20),
    ]);

    useFilesStore.getState().upsertFiles([
      file("old.ARW", 99),
      file("new.ARW", 30),
    ]);

    expect(useFilesStore.getState().files).toEqual([
      file("old.ARW", 99),
      file("keep.ARW", 20),
      file("new.ARW", 30),
    ]);
  });

  it("removes only explicitly selected object keys", () => {
    useFilesStore.getState().setFiles([
      file("remove.ARW", 10),
      file("keep.ARW", 20),
    ]);

    useFilesStore
      .getState()
      .removeFiles(["remove.ARW", "not-present.ARW"]);

    expect(useFilesStore.getState().files).toEqual([file("keep.ARW", 20)]);
  });
});
