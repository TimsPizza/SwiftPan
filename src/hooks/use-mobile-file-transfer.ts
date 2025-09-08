import type { FileItem as File } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { useTransferStore } from "@/store/transfer-store";
import { remove } from "@tauri-apps/plugin-fs";
import { useCallback } from "react";
import { toast } from "sonner";

export function useMobileFileTransfer(files?: File[]) {
  const setTransfersOpen = useTransferStore((s) => s.ui.setOpen);

  const pickUploads = useCallback(async () => {
    const isAndroid = /Android/i.test(navigator.userAgent || "");
    const bucketKeys = new Set((files || []).map((f) => f.id));
    const storeItems = useTransferStore.getState().items;

    if (isAndroid) {
      try {
        const picked = await nv.android_pick_upload_files();
        const entries = await picked.unwrapOr([] as any);
        if (!entries || entries.length === 0) {
          toast.error("No files selected");
          return;
        }
        let started = 0;
        for (const e of entries as any[]) {
          const key = String(e.name || "upload.bin");
          const uri = String(e.uri || "");
          if (!uri) continue;
          if (bucketKeys.has(key)) {
            toast.error(`File already exists: ${key}`);
            continue;
          }
          const dup = Object.values(storeItems).some(
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
          const res = await nv.android_upload_from_uri({
            key,
            uri,
            part_size: 8 * 1024 * 1024,
          });
          await res.match(
            () => {
              started++;
              setTransfersOpen(true);
            },
            (err) => {
              console.error(err);
              toast.error(`Upload failed to start: ${key}`);
            },
          );
        }
        // Regardless of started count, do not reopen another picker; exit.
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
      for (const f of filesChosen) {
        const key = f.name;
        if (bucketKeys.has(key)) {
          toast.error(`File already exists: ${key}`);
          continue;
        }
        const dup = Object.values(storeItems).some(
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
        const start = await nv.upload_new_stream({
          key,
          bytes_total: f.size,
          part_size: 8 * 1024 * 1024,
        });
        await start.match(
          async (id) => {
            setTransfersOpen(true);
            const CHUNK = 1024 * 1024 * 4;
            for (let offset = 0; offset < f.size; offset += CHUNK) {
              const slice = f.slice(offset, Math.min(f.size, offset + CHUNK));
              const buf = new Uint8Array(await slice.arrayBuffer());
              const r = await nv.upload_stream_write(id, buf);
              if (r.isErr()) break;
            }
            await nv.upload_stream_finish(id);
          },
          async (e) => {
            console.error(e);
            toast.error(`Upload failed to start: ${key}`);
          },
        );
      }
    };
    input.click();
  }, [files, setTransfersOpen]);

  // Helper to wait for a download to finish via polling
  const waitForDownloadComplete = async (id: string) => {
    for (let i = 0; i < 600; i++) {
      // up to ~60s
      try {
        const st = await nv.download_status(id).unwrapOr(null as any);
        const s = st as any;
        if (s && typeof s === "object") {
          const total = Number((s as any).bytes_total || 0);
          const done = Number((s as any).bytes_done || 0);
          if (total > 0 && done >= total) return true;
        }
      } catch {}
      await new Promise((r) => setTimeout(r, 100));
    }
    return false;
  };

  const ensureTreeUri = useCallback(async (): Promise<string | null> => {
    // Always prompt user to pick a download directory (no persistence)
    try {
      const result = await nv.android_pick_download_dir();
      const treeUri = await result.unwrapOr(null);
      if (!treeUri) {
        toast.error("No directory selected");
        return null;
      }
      return treeUri;
    } catch (e) {
      console.error(e);
      toast.error("Failed to open directory picker");
      return null;
    }
  }, []);

  const downloadOne = useCallback(
    async (file: File) => {
      const tree = await ensureTreeUri();
      if (!tree) return;
      const sandboxDir = await (await nv.download_sandbox_dir()).unwrapOr("");
      const base = String(sandboxDir || "");
      if (!base) {
        toast.error("No sandbox directory available");
        return;
      }
      const sandboxPath = `${base.replace(/[\\/]$/, "")}/${file.id}`;
      try {
        const res = await nv.download_new({
          key: file.id,
          dest_path: sandboxPath,
          chunk_size: 4 * 1024 * 1024,
        });
        await res.match(
          async (id) => {
            setTransfersOpen(true);
            const ok = await waitForDownloadComplete(String(id));
            if (!ok) throw new Error("download timeout");
            const copy = await nv.android_copy_from_path_to_tree({
              src_path: sandboxPath,
              tree_uri: tree,
              relative_path: file.filename || `download_${file.id}`,
              mime: undefined,
            });
            await copy.match(
              async () => {
                try {
                  await remove(sandboxPath);
                } catch {}
                toast.success(`Downloaded: ${file.filename}`);
              },
              (err) => {
                toast.error(`Failed to copy ${file.filename}: ${String(err)}`);
              },
            );
          },
          async (e) => {
            throw new Error(String((e as any)?.message || e));
          },
        );
      } catch (e) {
        toast.error(`Failed to download ${file.filename}`);
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
      toast.info(`Downloading ${many.length} files`);
      for (const file of many) {
        try {
          const sandboxDir = await (
            await nv.download_sandbox_dir()
          ).unwrapOr("");
          const base = String(sandboxDir || "");
          if (!base) throw new Error("No sandbox directory available");
          const sandboxPath = `${base.replace(/[\\/]$/, "")}/${file.id}`;
          const res = await nv.download_new({
            key: file.id,
            dest_path: sandboxPath,
            chunk_size: 4 * 1024 * 1024,
          });
          await res.match(
            async (id) => {
              setTransfersOpen(true);
              const ok = await waitForDownloadComplete(String(id));
              if (!ok) throw new Error("download timeout");
              const copy = await nv.android_copy_from_path_to_tree({
                src_path: sandboxPath,
                tree_uri: tree,
                relative_path: file.filename || `download_${file.id}`,
                mime: undefined,
              });
              await copy.match(
                async () => {
                  try {
                    await remove(sandboxPath);
                  } catch {}
                  success++;
                },
                () => {
                  fail++;
                },
              );
            },
            async () => {
              fail++;
            },
          );
        } catch {
          fail++;
        }
        await new Promise((r) => setTimeout(r, 200));
      }
      if (success) toast.success(`Successfully downloaded ${success} files`);
      if (fail) toast.error(`Failed to download ${fail} files`);
    },
    [ensureTreeUri, setTransfersOpen],
  );

  return { pickUploads, downloadOne, downloadMany } as const;
}
