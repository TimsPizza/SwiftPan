import { Button } from "@/components/ui/Button";
import { nv } from "@/lib/api/tauriBridge";
import { useEffect, useMemo, useState } from "react";

export default function UsagePage() {
  const [month, setMonth] = useState(() =>
    new Date().toISOString().slice(0, 7),
  );
  const [items, setItems] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const totals = useMemo(() => {
    return items.reduce(
      (acc, d) => {
        acc.ingress += d.ingress_bytes || 0;
        acc.egress += d.egress_bytes || 0;
        return acc;
      },
      { ingress: 0, egress: 0 },
    );
  }, [items]);

  const load = async () => {
    setLoading(true);
    setError(null);
    const res = await nv.usage_list_month(month);
    res.match(
      (ok) => setItems(ok),
      (e) => setError(String((e as any)?.message || e)),
    );
    setLoading(false);
  };

  useEffect(() => {
    // Auto merge today's deltas; backend will no-op if already merged today
    (async () => {
      const today = new Date().toISOString().slice(0, 10);
      await nv.usage_merge_day(today);
      await load();
    })();
  }, [month]);

  const mergeToday = async () => {
    const today = new Date().toISOString().slice(0, 10);
    const r = await nv.usage_merge_day(today);
    r.match(
      () => load(),
      (e) => setError(String((e as any)?.message || e)),
    );
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <input
          type="month"
          value={month}
          onChange={(e) => setMonth(e.target.value)}
          className="border-input bg-background ring-offset-background focus-visible:ring-ring placeholder:text-muted-foreground flex h-9 w-fit rounded-md border px-3 py-1 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
        />
        <Button onClick={load} disabled={loading}>
          Refresh
        </Button>
        <Button variant="outline" onClick={mergeToday}>
          Merge Today
        </Button>
        {loading && <span className="text-sm">Loading…</span>}
        {error && <span className="text-sm text-red-600">{error}</span>}
      </div>
      <div className="rounded border">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left">
              <th className="px-2 py-1">Date</th>
              <th className="px-2 py-1">Ingress</th>
              <th className="px-2 py-1">Egress</th>
              <th className="px-2 py-1">PeakStorage</th>
              <th className="px-2 py-1">DeletedStorage</th>
              <th className="px-2 py-1">Rev</th>
              <th className="px-2 py-1">Updated</th>
            </tr>
          </thead>
          <tbody>
            {items.map((d) => (
              <tr key={d.date} className="border-t">
                <td className="px-2 py-1">{d.date}</td>
                <td className="px-2 py-1">{d.ingress_bytes}</td>
                <td className="px-2 py-1">{d.egress_bytes}</td>
                <td className="px-2 py-1">{d.peak_storage_bytes ?? 0}</td>
                <td className="px-2 py-1">{d.deleted_storage_bytes ?? 0}</td>
                <td className="px-2 py-1">{d.rev}</td>
                <td className="px-2 py-1">{d.updated_at}</td>
              </tr>
            ))}
          </tbody>
          <tfoot>
            <tr className="border-t font-medium">
              <td className="px-2 py-1">Total</td>
              <td className="px-2 py-1">{totals.ingress}</td>
              <td className="px-2 py-1">{totals.egress}</td>
              <td className="px-2 py-1" colSpan={4}></td>
            </tr>
          </tfoot>
        </table>
      </div>
    </div>
  );
}
