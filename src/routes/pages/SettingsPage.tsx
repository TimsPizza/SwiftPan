import { Button } from "@/components/ui/Button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  SettingsPatchSchema,
  type SettingsFormValues,
} from "@/lib/api/schemas";
import { mutations, nv, queries } from "@/lib/api/tauriBridge";
import { useAppStore } from "@/store/app-store";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useQueryClient } from "react-query";
import { toast } from "sonner";

export default function SettingsPage() {
  const queryClient = useQueryClient();
  // App settings via store (populated by EventBridge)
  const {
    logLevel,
    maxConcurrency,
    defaultDownloadDir,
    uploadThumbnail,
    setLogLevel,
    setMaxConcurrency,
    setUploadThumbnail,
  } = useAppStore();
  const {
    register,
    handleSubmit,
    setError,
    formState: { errors },
  } = useForm<SettingsFormValues>({
    defaultValues: {
      endpoint: "",
      access_key_id: "",
      secret_access_key: "",
      bucket: "",
      region: "auto",
    },
    mode: "onBlur",
  });
  const [msg, setMsg] = useState<{
    msg: string;
    isError: boolean;
  } | null>(null);
  const [redacted, setRedacted] = useState<null | {
    endpoint: string;
    access_key_id: string;
    secret_access_key: string;
    bucket: string;
    region?: string;
  }>(null);

  const statusQ = queries.useBackendStatus();
  const credsQ = queries.useBackendCredentialsRedacted({
    onSuccess: (ok) => setRedacted(ok),
    onError: () => setRedacted(null),
  });

  const save = handleSubmit(async (values) => {
    setMsg({ msg: "", isError: false });
    // Validate with zod
    // Build a partial patch from non-empty fields
    const patch: Partial<SettingsFormValues> = {};
    if (values.endpoint && values.endpoint.trim().length > 0)
      patch.endpoint = values.endpoint.trim();
    if (values.access_key_id && values.access_key_id.trim().length > 0)
      patch.access_key_id = values.access_key_id.trim();
    if (values.secret_access_key && values.secret_access_key.trim().length > 0)
      patch.secret_access_key = values.secret_access_key.trim();
    if (values.bucket && values.bucket.trim().length > 0)
      patch.bucket = values.bucket.trim();
    if (values.region && values.region.trim().length > 0)
      patch.region = values.region.trim();

    const parsed = SettingsPatchSchema.safeParse(patch);
    if (!parsed.success) {
      parsed.error.issues.forEach((iss) => {
        const path = (iss.path?.join(".") || "") as keyof SettingsFormValues;
        setError(path, { type: "zod", message: iss.message });
      });
      return;
    }
    await saveMutation.mutateAsync(parsed.data);
    queryClient.invalidateQueries(["list_all_objects"]);
    await Promise.all([statusQ.refetch(), credsQ.refetch()]);
    toast.success("Settings saved");
  });

  // Unlock flow removed — vault has no lock semantics now

  const saveMutation = mutations.useSaveCredentials({
    onError: (e: any) =>
      setMsg({ msg: String(e?.message || e), isError: true }),
  });

  const sanityMutation = mutations.useR2Sanity({
    onSuccess: () => void toast.success("R2 connectivity OK"),
    onError: (e: any) => {
      // eslint-disable-next-line no-console
      console.error("[ui] r2_sanity_check error", e);
      setMsg({ msg: String(e?.message || e), isError: true });
    },
  });

  return (
    <div className="space-y-3">
      <Tabs defaultValue="creds" className="">
        <TabsList className="grid w-full grid-cols-2 md:max-w-[224px]">
          <TabsTrigger value="creds">R2 Credentials</TabsTrigger>
          <TabsTrigger value="app">App Settings</TabsTrigger>
        </TabsList>
        <TabsContent value="creds" className="flex flex-col gap-2">
          <form onSubmit={save} className="grid max-w-xl grid-cols-2 gap-2">
            <label className="text-sm">Endpoint</label>
            <div>
              <input
                className="w-full rounded border px-2 py-1"
                placeholder={
                  redacted?.endpoint ||
                  "https://<account>.r2.cloudflarestorage.com"
                }
                {...register("endpoint")}
              />
              {errors.endpoint && (
                <div className="mt-1 text-xs text-red-600">
                  {errors.endpoint.message}
                </div>
              )}
            </div>

            <label className="text-sm">Access Key ID</label>
            <div>
              <input
                className="w-full rounded border px-2 py-1"
                placeholder={redacted?.access_key_id || "Access Key ID"}
                {...register("access_key_id")}
              />
              {errors.access_key_id && (
                <div className="mt-1 text-xs text-red-600">
                  {errors.access_key_id.message}
                </div>
              )}
            </div>

            <label className="text-sm">Secret Access Key</label>
            <div>
              <input
                className="w-full rounded border px-2 py-1"
                type="password"
                placeholder={redacted?.secret_access_key || "Secret Access Key"}
                {...register("secret_access_key")}
              />
              {errors.secret_access_key && (
                <div className="mt-1 text-xs text-red-600">
                  {errors.secret_access_key.message}
                </div>
              )}
            </div>

            <label className="text-sm">Bucket</label>
            <div>
              <input
                className="w-full rounded border px-2 py-1"
                placeholder={redacted?.bucket || "Bucket"}
                {...register("bucket")}
              />
              {errors.bucket && (
                <div className="mt-1 text-xs text-red-600">
                  {errors.bucket.message}
                </div>
              )}
            </div>

            <label className="text-sm">Region</label>
            <div>
              <input
                className="w-full rounded border px-2 py-1"
                placeholder={redacted?.region || "Region"}
                {...register("region")}
              />
              {errors.region && (
                <div className="mt-1 text-xs text-red-600">
                  {errors.region.message}
                </div>
              )}
            </div>

            {/* Device ID and Master Password removed */}
          </form>
          <div className="flex gap-2">
            <Button
              type="submit"
              form=""
              onClick={save}
              disabled={saveMutation.isLoading}
            >
              {saveMutation.isLoading ? "Saving…" : "Save Credentials"}
            </Button>
            <Button
              variant="outline"
              onClick={() => sanityMutation.mutate()}
              disabled={sanityMutation.isLoading}
            >
              {sanityMutation.isLoading ? "Testing…" : "Test Connection"}
            </Button>
          </div>
        </TabsContent>
        <TabsContent value="app">
          <div className="mt-4 grid max-w-xl grid-cols-2 gap-2">
            <label className="text-sm">Log level</label>
            <div className="flex items-center gap-2">
              <Select
                className="w-full rounded border px-2 py-1"
                value={logLevel}
                onValueChange={(v: string) => {
                  setLogLevel(v);
                  void nv.log_set_level(v as any);
                }}
              >
                <SelectTrigger>
                  <SelectValue placeholder="log level" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="trace">trace</SelectItem>
                  <SelectItem value="debug">debug</SelectItem>
                  <SelectItem value="info">info</SelectItem>
                  <SelectItem value="warn">warn</SelectItem>
                  <SelectItem value="error">error</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <label className="text-sm">Max concurrency</label>
            <input
              className="w-full rounded border px-2 py-1"
              type="number"
              min={1}
              max={16}
              value={maxConcurrency}
              onChange={(e) => setMaxConcurrency(Number(e.target.value) || 1)}
            />
            {/* deprecated */}
            {/* <label className="text-sm">Default download directory</label>
            <div className="flex items-center gap-2">
              <input
                className="w-full rounded border px-2 py-1"
                placeholder="/path/to/downloads"
                value={defaultDownloadDir || ""}
                onChange={(e) => setDefaultDownloadDir(e.target.value || null)}
              />
              <Button
                type="button"
                variant="outline"
                onClick={async () => {
                  const { open } = await import("@tauri-apps/plugin-dialog");
                  const pick = await open({ directory: true, multiple: false });
                  if (pick) setDefaultDownloadDir(String(pick));
                }}
              >
                Choose…
              </Button>
            </div> */}

            <label className="text-sm">{`Upload thumbnail alongside file (Not implementd)`}</label>
            <div>
              <Switch
                id="upload-thumb"
                checked={!!uploadThumbnail}
                onCheckedChange={() => setUploadThumbnail(!uploadThumbnail)}
              />
            </div>
            <div className="flex gap-2">
              <Button
                onClick={async () => {
                  const r = await nv.settings_set({
                    logLevel,
                    maxConcurrency,
                    defaultDownloadDir: defaultDownloadDir || undefined,
                    uploadThumbnail,
                  });
                  r.match(
                    (_ok) => {
                      toast.success("Settings saved");
                    },
                    (_err) => {
                      toast.error("Failed to save settings");
                    },
                  );
                }}
              >
                Save App Settings
              </Button>
            </div>
          </div>
        </TabsContent>
      </Tabs>
      {(statusQ.isLoading || credsQ.isLoading) && (
        <div className="text-sm">Loading status…</div>
      )}
      {/* no longer show success msg */}
      {msg && (
        <div
          className={`text-sm ${msg.isError ? "text-red-600" : "text-green-700"}`}
        >
          {msg.isError && msg.msg}
        </div>
      )}
    </div>
  );
}
