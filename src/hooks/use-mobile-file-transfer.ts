import type { FileItem as File } from "@/lib/api/schemas";
import { api } from "@/lib/api/tauriBridge";
import { useAppStore } from "@/store/app-store";
import { useFilesStore } from "@/store/files-store";
import { useTransferStore } from "@/store/transfer-store";
import { useCallback } from "react";
import { toast } from "sonner";

export function useMobileFileTransfer(filesOverride?: File[]) {
  const setTransfersOpen = useTransferStore((s) => s.ui.setOpen);
  const androidTreeUri = useAppStore((s) => s.androidTreeUri);
  const setAndroidTreeUri = useAppStore((s) => s.setAndroidTreeUri);
  const allFiles = useFilesStore((s) => s.files);
  const files = filesOverride ?? allFiles;

  const pickUploads = useCallback(async () => {
    const isAndroid = /Android/i.test(navigator.userAgent || "");
    const existingObjectKeys = new Set((files || []).map((f) => f.id));

    if (isAndroid) {
      try {
        const entries = await api.android_pick_upload_files();
        if (!entries || entries.length === 0) {
          toast.error("No files selected");
          return;
        }
        let started = 0;
        const pendingObjectKeys = new Set(existingObjectKeys);
        for (const e of entries as any[]) {
          const key =
            String(e.displayName || "upload.bin").trim() || "upload.bin";
          const uri = String(e.uri || "");
          if (!uri) continue;
          if (pendingObjectKeys.has(key)) {
            toast.error(`File already exists: ${key}`);
            continue;
          }
          const dup = Object.values(useTransferStore.getState().items).some(
            (t) =>
              t.type === "upload" &&
              t.key === key &&
              t.state !== "completed" &&
              t.state !== "failed",
          );
          if (dup) {
            toast.info(`Already uploading: ${key}`);
            continue;
          }
          try {
            await api.android_upload_from_uri({
              key,
              uri,
              part_size: 8 * 1024 * 1024,
            });
            started += 1;
            pendingObjectKeys.add(key);
            setTransfersOpen(true);
          } catch (err) {
            console.error(err);
            toast.error(`Upload failed to start: ${key}`);
          }
        }

        if (started === 0) {
          toast.error("No uploads started");
        }

        return;
      } catch (e) {
        console.error(e);
        // On native error, do not fallback to avoid double dialogs
        toast.error("Failed to open file picker");
        return;
      }
    }

    // Fallback: HTML input + JS streaming
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.onchange = async () => {
      const filesChosen = Array.from(input.files || []);
      if (filesChosen.length === 0) return;
      const pendingObjectKeys = new Set(existingObjectKeys);
      for (const f of filesChosen) {
        const key = f.name;
        if (pendingObjectKeys.has(key)) {
          toast.error(`File already exists: ${key}`);
          continue;
        }
        const dup = Object.values(useTransferStore.getState().items).some(
          (t) =>
            t.type === "upload" &&
            t.key === key &&
            t.state !== "completed" &&
            t.state !== "failed",
        );
        if (dup) {
          toast.info(`Already uploading: ${key}`);
          continue;
        }
        try {
          const id = await api.upload_new_stream({
            key,
            bytes_total: f.size,
            part_size: 8 * 1024 * 1024,
          });
          pendingObjectKeys.add(key);
          setTransfersOpen(true);
          const CHUNK = 1024 * 1024 * 4;
          for (let offset = 0; offset < f.size; offset += CHUNK) {
            const slice = f.slice(offset, Math.min(f.size, offset + CHUNK));
            const buf = new Uint8Array(await slice.arrayBuffer());
            try {
              await api.upload_stream_write(id, buf);
            } catch (err) {
              console.error("upload_stream_write failed", err);
              toast.error(`Upload chunk failed for ${key}`);
              break;
            }
          }
          await api.upload_stream_finish(id);
        } catch (e) {
          console.error(e);
          toast.error(`Upload failed to start: ${key}`);
        }
      }
    };
    input.click();
  }, [files, setTransfersOpen]);

  const ensureTreeUri = useCallback(async (): Promise<string | null> => {
    if (androidTreeUri) return androidTreeUri;

    try {
      const persisted = await api.android_get_persisted_download_dir();
      if (persisted) {
        setAndroidTreeUri(persisted);
        return persisted;
      }
    } catch (err) {
      console.error(err);
    }

    try {
      const treeUri = await api.android_pick_download_dir();
      if (!treeUri) {
        toast.error("No directory selected");
        return null;
      }
      setAndroidTreeUri(treeUri);
      toast.success("Download directory saved");
      return treeUri;
    } catch (e) {
      console.error(e);
      toast.error("Failed to open directory picker");
      return null;
    }
  }, [androidTreeUri, setAndroidTreeUri]);

  const downloadOne = useCallback(
    async (file: File) => {
      const tree = await ensureTreeUri();
      if (!tree) return;
      try {
        await api.download_new({
          key: file.id,
          chunk_size: 4 * 1024 * 1024,
          android_tree_uri: tree,
          android_relative_path: file.filename || `download_${file.id}`,
        });
        setTransfersOpen(true);
      } catch (e) {
        console.error(e);
        toast.error(`Failed to start download: ${file.filename}`);
      }
    },
    [ensureTreeUri, setTransfersOpen],
  );

  const downloadMany = useCallback(
    async (many: File[]) => {
      if (!many || many.length === 0) return;
      const tree = await ensureTreeUri();
      if (!tree) return;
      let success = 0,
        fail = 0;
      toast.info(`Starting ${many.length} downloads`);
      for (const file of many) {
        try {
          await api.download_new({
            key: file.id,
            chunk_size: 4 * 1024 * 1024,
            android_tree_uri: tree,
            android_relative_path: file.filename || `download_${file.id}`,
          });
          setTransfersOpen(true);
          success++;
        } catch (err) {
          console.error(err);
          fail++;
        }
        await new Promise((r) => setTimeout(r, 200));
      }
      if (success) toast.success(`Started ${success} downloads`);
      if (fail) toast.error(`Failed to start ${fail} downloads`);
    },
    [ensureTreeUri, setTransfersOpen],
  );

  return { pickUploads, downloadOne, downloadMany } as const;
}
