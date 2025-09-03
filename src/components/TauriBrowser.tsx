import { useEffect, useState } from "react";
import { list_objects, delete_object, share_generate, ANALYTICS_PREFIX } from "@/lib/api/tauriBridge";
import { Button } from "@/components/ui/Button";

type Item = {
  key: string;
  size?: number;
  last_modified_ms?: number;
  etag?: string;
  is_prefix: boolean;
  protected: boolean;
};

export default function TauriBrowser() {
  const [prefix, setPrefix] = useState("");
  const [items, setItems] = useState<Item[]>([]);
  const [token, setToken] = useState<string | undefined>();
  const [next, setNext] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = async (p = prefix, t?: string) => {
    setLoading(true);
    setError(null);
    try {
      const res = await list_objects(p, t, 200);
      setItems(res.items);
      setNext(res.next_token);
      setToken(t);
    } catch (e: any) {
      setError(String(e?.message || e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(""); }, []);

  const enter = (it: Item) => {
    if (!it.is_prefix) return;
    setPrefix(it.key);
    load(it.key);
  };

  const up = () => {
    if (!prefix) return;
    const parts = prefix.split("/").filter(Boolean);
    parts.pop();
    const p = parts.length ? parts.join("/") + "/" : "";
    setPrefix(p);
    load(p);
  };

  const del = async (it: Item) => {
    if (it.protected || it.key.startsWith(ANALYTICS_PREFIX)) return alert("Analytics files are protected.");
    if (!confirm(`Delete ${it.key}?`)) return;
    await delete_object(it.key);
    load(prefix, token);
  };

  const copy = async (it: Item) => {
    const r = await share_generate({ key: it.key, ttl_secs: 3600, download_filename: undefined });
    await navigator.clipboard.writeText(r.url);
    alert("Link copied");
  };

  return (
    <div className="p-4 space-y-2">
      <div className="flex items-center gap-2">
        <Button onClick={up} disabled={!prefix || loading}>Up</Button>
        <div className="text-sm text-muted-foreground">Prefix: {prefix || "/"}</div>
        {loading && <div className="text-sm">Loading…</div>}
        {error && <div className="text-sm text-red-600">{error}</div>}
      </div>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left">
            <th className="py-1">Name</th>
            <th className="py-1 w-24">Size</th>
            <th className="py-1 w-64">Actions</th>
          </tr>
        </thead>
        <tbody>
          {items.map((it) => (
            <tr key={it.key} className="border-t">
              <td className="py-1">
                {it.is_prefix ? (
                  <button className="text-blue-600" onClick={() => enter(it)}>{it.key}</button>
                ) : (
                  <span className={it.protected ? "text-gray-500" : ""}>{it.key}</span>
                )}
              </td>
              <td className="py-1">{it.is_prefix ? "" : (it.size ?? 0)}</td>
              <td className="py-1 flex gap-2">
                {!it.is_prefix && (
                  <>
                    <Button variant="outline" size="sm" onClick={() => copy(it)}>Copy Link</Button>
                    <Button variant="destructive" size="sm" onClick={() => del(it)} disabled={it.protected}>Delete</Button>
                  </>
                )}
                {it.is_prefix && <span className="text-muted-foreground">Folder</span>}
                {it.protected && <span className="text-xs text-amber-700">Protected analytics</span>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {next && (
        <div>
          <Button onClick={() => load(prefix, next)}>Load more</Button>
        </div>
      )}
    </div>
  );
}

