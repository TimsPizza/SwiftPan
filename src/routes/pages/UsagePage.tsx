import { TrafficTrendsChart } from "@/components/charts/TrafficTrendsChart";
import { UsageTrendsChart } from "@/components/charts/UsageTrendsChart";
import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { nv } from "@/lib/api/tauriBridge";
import { useEffect, useMemo, useState } from "react";

export default function UsagePage() {
  const [month, setMonth] = useState(() =>
    new Date().toISOString().slice(0, 7),
  );
  const [items, setItems] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cost, setCost] = useState<any | null>(null);
  const [costLoading, setCostLoading] = useState(false);
  const [costError, setCostError] = useState<string | null>(null);

  const trafficPoints = useMemo(
    () =>
      (items || []).map((d) => ({
        date: d.date as string,
        uploadBytes: Number(d.ingress_bytes || 0),
        downloadBytes: Number(d.egress_bytes || 0),
      })),
    [items],
  );

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

  const loadCost = async () => {
    setCostLoading(true);
    setCostError(null);
    const res = await nv.usage_month_cost(month);
    res.match(
      (ok) => setCost(ok),
      (e) => setCostError(String((e as any)?.message || e)),
    );
    setCostLoading(false);
  };

  useEffect(() => {
    // Auto merge today's deltas; backend will no-op if already merged today
    (async () => {
      const today = new Date().toISOString().slice(0, 10);
      await nv.usage_merge_day(today);
      await Promise.all([load(), loadCost()]);
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
      <Card>
        <CardHeader>
          <CardTitle>Monthly Usage Report</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <input
              type="month"
              value={month}
              onChange={(e) => setMonth(e.target.value)}
              className="border-input bg-background ring-offset-background focus-visible:ring-ring placeholder:text-muted-foreground flex h-9 w-fit rounded-md border px-3 py-1 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
            />
            <Button
              onClick={() => Promise.all([load(), loadCost()])}
              disabled={loading || costLoading}
            >
              Refresh
            </Button>
            <Button
              variant="outline"
              onClick={async () => {
                await mergeToday();
                await loadCost();
              }}
            >
              Merge Today
            </Button>
            {(loading || costLoading) && (
              <span className="text-sm">Loading…</span>
            )}
            {(error || costError) && (
              <span className="text-sm text-red-600">{error || costError}</span>
            )}
          </div>
          {cost && (
            <div className="grid grid-cols-1 gap-4">
              {/* Storage */}
              <div>
                <div className="mb-1 flex items-center justify-between text-sm">
                  <span>Storage</span>
                  <span className="text-muted-foreground">
                    {Number(cost.storage?.avg_gb_month_ceil ?? 0).toFixed(0)} /
                    {Number(cost.storage?.free_gb_month ?? 10).toFixed(0)}{" "}
                    GB-month
                  </span>
                </div>
                <Progress
                  value={(() => {
                    const cur = Number(cost.storage?.avg_gb_month_ceil ?? 0);
                    const tot = cur + Number(cost.storage?.free_gb_month ?? 10);
                    return tot > 0 ? Math.min(100, (cur / tot) * 100) : 0;
                  })()}
                />
              </div>
              {/* Class A */}
              <div>
                <div className="mb-1 flex items-center justify-between text-sm">
                  <span>Class A</span>
                  <span className="text-muted-foreground">
                    {Number(cost.class_a?.total_ops ?? 0).toLocaleString()} /
                    {Number(
                      cost.class_a?.free_ops ?? 1_000_000,
                    ).toLocaleString()}{" "}
                    ops
                  </span>
                </div>
                <Progress
                  value={(() => {
                    const cur = Number(cost.class_a?.total_ops ?? 0);
                    const tot = Number(cost.class_a?.free_ops ?? 1_000_000);
                    return tot > 0 ? Math.min(100, (cur / tot) * 100) : 0;
                  })()}
                />
              </div>
              {/* Class B */}
              <div>
                <div className="mb-1 flex items-center justify-between text-sm">
                  <span>Class B</span>
                  <span className="text-muted-foreground">
                    {Number(cost.class_b?.total_ops ?? 0).toLocaleString()} /
                    {Number(
                      cost.class_b?.free_ops ?? 10_000_000,
                    ).toLocaleString()}{" "}
                    ops
                  </span>
                </div>
                <Progress
                  value={(() => {
                    const cur = Number(cost.class_b?.total_ops ?? 0);
                    const tot = Number(cost.class_b?.free_ops ?? 10_000_000);
                    return tot > 0 ? Math.min(100, (cur / tot) * 100) : 0;
                  })()}
                />
              </div>
              <div className="text-muted-foreground text-sm">
                <div>
                  Month: {cost.month} · Total Cost: $
                  {Number(cost.total_cost_usd ?? 0).toFixed(2)}
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Monthly Usage Chart (from backend daily ledgers) */}
      <Card>
        <CardHeader>
          <CardTitle>Usage</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-64 w-full">
            <UsageTrendsChart points={items} />
          </div>
        </CardContent>
      </Card>

      {/* Monthly Traffic Chart (from backend daily ledgers) */}
      <Card>
        <CardHeader>
          <CardTitle>Traffic</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-64 w-full">
            <TrafficTrendsChart points={trafficPoints as any} />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
