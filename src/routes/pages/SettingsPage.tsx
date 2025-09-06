import { Button } from "@/components/ui/Button";
import {
  SettingsPatchSchema,
  type SettingsFormValues,
} from "@/lib/api/schemas";
import { mutations, nv, queries } from "@/lib/api/tauriBridge";
import { useState } from "react";
import { useForm } from "react-hook-form";

export default function SettingsPage() {
  const appSettingsQ = queries.useSettings();
  const [logLevel, setLogLevel] = useState<string>(
    appSettingsQ.data?.logLevel || "info",
  );
  const [maxConcurrency, setMaxConcurrency] = useState<number>(
    appSettingsQ.data?.maxConcurrency || 2,
  );
  const [defaultDownloadDir, setDefaultDownloadDir] = useState<string>(
    appSettingsQ.data?.defaultDownloadDir || "",
  );
  const [uploadThumbnail, setUploadThumbnail] = useState<boolean>(
    appSettingsQ.data?.uploadThumbnail ?? false,
  );
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
    if (
      values.secret_access_key &&
      values.secret_access_key.trim().length > 0
    )
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
    await Promise.all([statusQ.refetch(), credsQ.refetch()]);
    setMsg({ msg: "Saved", isError: false });
  });

  // Unlock flow removed — vault has no lock semantics now

  const saveMutation = mutations.useSaveCredentials({
    onError: (e: any) =>
      setMsg({ msg: String(e?.message || e), isError: true }),
  });

  const sanityMutation = mutations.useR2Sanity({
    onSuccess: () => setMsg({ msg: "R2 connectivity OK", isError: false }),
    onError: (e: any) => {
      // eslint-disable-next-line no-console
      console.error("[ui] r2_sanity_check error", e);
      setMsg({ msg: String(e?.message || e), isError: true });
    },
  });

  return (
    <div className="space-y-3">
      {/* Vault lock semantics removed; show simple configured hint if needed */}
      {/* <div className="text-muted-foreground text-sm">Configured</div> */}
      <form onSubmit={save} className="grid max-w-xl grid-cols-2 gap-2">
        <label className="text-sm">Endpoint</label>
        <div>
          <input
            className="w-full rounded border px-2 py-1"
            placeholder={
              redacted?.endpoint || "https://<account>.r2.cloudflarestorage.com"
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
      <div className="mt-4 grid max-w-xl grid-cols-2 gap-2">
        <label className="text-sm">Log level</label>
        <select
          className="w-full rounded border px-2 py-1"
          value={logLevel}
          onChange={(e) => {
            setLogLevel(e.target.value);
            void nv.log_set_level(e.target.value as any);
          }}
        >
          <option value="trace">trace</option>
          <option value="debug">debug</option>
          <option value="info">info</option>
          <option value="warn">warn</option>
          <option value="error">error</option>
        </select>

        <label className="text-sm">Max concurrency</label>
        <input
          className="w-full rounded border px-2 py-1"
          type="number"
          min={1}
          max={16}
          value={maxConcurrency}
          onChange={(e) => setMaxConcurrency(Number(e.target.value) || 1)}
        />

        <label className="text-sm">Default download directory</label>
        <input
          className="w-full rounded border px-2 py-1"
          placeholder="/path/to/downloads"
          value={defaultDownloadDir}
          onChange={(e) => setDefaultDownloadDir(e.target.value)}
        />

        <label className="text-sm">Upload thumbnail alongside file</label>
        <div>
          <input
            id="upload-thumb"
            type="checkbox"
            className="mr-2"
            checked={uploadThumbnail}
            onChange={(e) => setUploadThumbnail(e.target.checked)}
          />
          <label htmlFor="upload-thumb" className="text-sm">
            Enable
          </label>
        </div>

        <div />
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={async () => {
              await (
                await nv.settings_set({
                  logLevel,
                  maxConcurrency,
                  defaultDownloadDir: defaultDownloadDir || undefined,
                  uploadThumbnail,
                })
              ).unwrapOr(undefined);
              setMsg({ msg: "Settings saved", isError: false });
            }}
          >
            Save App Settings
          </Button>
        </div>
      </div>
      <div className="flex gap-2">
        <Button
          type="submit"
          form=""
          onClick={save}
          disabled={saveMutation.isLoading}
        >
          {saveMutation.isLoading ? "Saving…" : "Save Bundle"}
        </Button>
        <Button
          variant="outline"
          onClick={() => sanityMutation.mutate()}
          disabled={sanityMutation.isLoading}
        >
          {sanityMutation.isLoading ? "Testing…" : "Test Connection"}
        </Button>
      </div>
      {(statusQ.isLoading || credsQ.isLoading) && (
        <div className="text-sm">Loading status…</div>
      )}
      {msg && (
        <div
          className={`text-sm ${msg.isError ? "text-red-600" : "text-green-700"}`}
        >
          {msg.msg}
        </div>
      )}
    </div>
  );
}
