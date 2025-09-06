import { Button } from "@/components/ui/Button";
import { SettingsSchema, type SettingsFormValues } from "@/lib/api/schemas";
import { mutations, queries } from "@/lib/api/tauriBridge";
import { useState } from "react";
import { useForm } from "react-hook-form";

export default function SettingsPage() {
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
    const parsed = SettingsSchema.safeParse(values);
    if (!parsed.success) {
      parsed.error.issues.forEach((iss) => {
        const path = (iss.path?.join(".") || "") as keyof SettingsFormValues;
        setError(path, { type: "zod", message: iss.message });
      });
      return;
    }
    const v = parsed.data;
    const bundle = {
      r2: {
        endpoint: v.endpoint,
        access_key_id: v.access_key_id,
        secret_access_key: v.secret_access_key,
        bucket: v.bucket,
        region: v.region,
      },
    };
    await saveMutation.mutateAsync(bundle);
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
