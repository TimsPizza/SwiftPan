import { Button } from "@/components/ui/Button";
import { SettingsSchema, type SettingsFormValues } from "@/lib/api/schemas";
import { nv } from "@/lib/api/tauriBridge";
import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";

export default function SettingsPage() {
  const {
    register,
    handleSubmit,
    setError,
    formState: { errors },
    getValues,
  } = useForm<SettingsFormValues>({
    defaultValues: {
      endpoint: "",
      access_key_id: "",
      secret_access_key: "",
      bucket: "",
      region: "auto",
      device_id: "dev-local",
      master_password: "",
    },
    mode: "onBlur",
  });
  const [status, setStatus] = useState<any>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const refreshStatus = async () => {
    const r = await nv.backend_status();
    r.match(
      (s) => setStatus(s as any),
      (e) => setMsg(String((e as any)?.message || e)),
    );
  };

  useEffect(() => {
    refreshStatus();
  }, []);

  const save = handleSubmit(async (values) => {
    setMsg(null);
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
      device_id: v.device_id,
      created_at: Date.now(),
    };
    const res = await nv.backend_set_credentials(bundle, v.master_password);
    res.match(
      async () => {
        await refreshStatus();
        setMsg("Saved");
      },
      (e) => {
        setMsg(String((e as any)?.message || e));
      },
    );
  });

  // Unlock flow removed — vault has no lock semantics now

  const sanity = async () => {
    setMsg(null);
    const res = await nv.r2_sanity_check();
    res.match(
      () => setMsg("R2 connectivity OK"),
      (e) => {
        // eslint-disable-next-line no-console
        console.error("[ui] r2_sanity_check error", e);
        setMsg(String((e as any)?.message || e));
      },
    );
  };

  return (
    <div className="space-y-3">
      {/* Vault lock semantics removed; show simple configured hint if needed */}
      {/* <div className="text-muted-foreground text-sm">Configured</div> */}
      <form onSubmit={save} className="grid max-w-xl grid-cols-2 gap-2">
        <label className="text-sm">Endpoint</label>
        <div>
          <input
            className="w-full rounded border px-2 py-1"
            placeholder="https://<account>.r2.cloudflarestorage.com"
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
            {...register("region")}
          />
          {errors.region && (
            <div className="mt-1 text-xs text-red-600">
              {errors.region.message}
            </div>
          )}
        </div>

        <label className="text-sm">Device ID</label>
        <div>
          <input
            className="w-full rounded border px-2 py-1"
            {...register("device_id")}
          />
          {errors.device_id && (
            <div className="mt-1 text-xs text-red-600">
              {errors.device_id.message}
            </div>
          )}
        </div>

        <label className="text-sm">Master Password</label>
        <div>
          <input
            className="w-full rounded border px-2 py-1"
            type="password"
            {...register("master_password")}
          />
          {errors.master_password && (
            <div className="mt-1 text-xs text-red-600">
              {errors.master_password.message}
            </div>
          )}
        </div>
      </form>
      <div className="flex gap-2">
        <Button type="submit" form="" onClick={save}>
          Save Bundle
        </Button>
        {/* Unlock removed */}
        <Button variant="outline" onClick={sanity}>
          R2 Sanity
        </Button>
      </div>
      {msg && <div className="text-sm text-green-700">{msg}</div>}
    </div>
  );
}
