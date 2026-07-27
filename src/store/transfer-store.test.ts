import { beforeEach, describe, expect, it } from "vitest";
import { useTransferStore, type TransferItem } from "./transfer-store";

function transfer(
  id: string,
  state: TransferItem["state"],
): TransferItem {
  return {
    id,
    type: "download",
    key: `${id}.ARW`,
    bytesDone: 0,
    rateBps: 0,
    state,
  };
}

describe("transfer store", () => {
  beforeEach(() => {
    useTransferStore.setState({
      items: {},
      ui: {
        ...useTransferStore.getState().ui,
        open: false,
      },
    });
  });

  it("patches an existing transfer without losing recovery fields", () => {
    useTransferStore.getState().upsert({
      ...transfer("download-1", "running"),
      destPath: "/downloads/file.ARW",
      tempPath: "/downloads/file.ARW.part",
      bytesTotal: 100,
    });

    useTransferStore.getState().update("download-1", {
      bytesDone: 40,
      state: "paused",
    });

    expect(useTransferStore.getState().items["download-1"]).toMatchObject({
      id: "download-1",
      destPath: "/downloads/file.ARW",
      tempPath: "/downloads/file.ARW.part",
      bytesTotal: 100,
      bytesDone: 40,
      state: "paused",
    });
  });

  it("clears completed transfers but preserves failures for diagnosis", () => {
    useTransferStore.getState().upsert(transfer("done", "completed"));
    useTransferStore.getState().upsert(transfer("failed", "failed"));
    useTransferStore.getState().upsert(transfer("paused", "paused"));

    useTransferStore.getState().clearCompleted();

    expect(Object.keys(useTransferStore.getState().items).sort()).toEqual([
      "failed",
      "paused",
    ]);
  });

  it("toggles the manager without changing transfer items", () => {
    useTransferStore.getState().upsert(transfer("download-1", "running"));

    useTransferStore.getState().ui.toggle();

    expect(useTransferStore.getState().ui.open).toBe(true);
    expect(useTransferStore.getState().items["download-1"]).toBeDefined();
  });
});
