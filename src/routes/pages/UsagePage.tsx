import { TrafficTrendsChart } from "@/components/charts/TrafficTrendsChart";
import { UsageTrendsChart } from "@/components/charts/UsageTrendsChart";
import GlobalError from "@/components/fallback/GlobalError";
import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { mutations, queries } from "@/lib/api/tauriBridge";
import { useEffect, useMemo, useState } from "react";

export default function UsagePage() {
  const [month, setMonth] = useState(() =>
    new Date().toISOString().slice(0, 7),
  );
  const [items, setItems] = useState<any[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [cost, setCost] = useState<any | null>(null);

  const trafficPoints = useMemo(
    () =>
      (items || []).map((d) => ({
        date: d.date as string,
        uploadBytes: Number(d.ingress_bytes || 0),
        downloadBytes: Number(d.egress_bytes || 0),
      })),
    [items],
  );

  const usageQ = queries.useUsageListMonth(month, {
    onSuccess: (ok) => setItems(ok as any[]),
    onError: (e: any) => setError(String(e?.message || e)),
  });
  const costQ = queries.useUsageMonthCost(month, {
    onSuccess: (ok) => setCost(ok),
    onError: (e: any) => setError(String(e?.message || e)),
  });

  useEffect(() => {
    // Auto merge today's deltas; backend will no-op if already merged today
    const today = new Date().toISOString().slice(0, 10);
    mergeMutation.mutate(today, {
      onSettled: () => {
        void Promise.all([usageQ.refetch(), costQ.refetch()]);
      },
    });
  }, [month]);

  const mergeMutation = mutations.useUsageMergeDay({
    onError: (e: any) => setError(String(e?.message || e)),
    onSuccess: () => usageQ.refetch(),
  });

  if (error) {
    const msg = String(error || "");
    const isUninit = /credentials|not.*found|uninitialized|backend|vault/i.test(
      msg,
    );
    return (
      <GlobalError
        title={isUninit ? "SwiftPan is not initialized" : "Cannot load usage"}
        description={
          isUninit
            ? "You need to configure your R2 credentials before viewing Usage."
            : msg
        }
        primaryLabel={isUninit ? "Go to Settings" : undefined}
        onPrimary={
          isUninit ? () => (window.location.href = "/settings") : undefined
        }
        secondaryLabel={isUninit ? "Retry" : undefined}
        onSecondary={isUninit ? () => window.location.reload() : undefined}
      />
    );
  }

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
              onClick={() => Promise.all([usageQ.refetch(), costQ.refetch()])}
              disabled={usageQ.isLoading || costQ.isLoading}
            >
              Refresh
            </Button>
            <Button
              variant="outline"
              onClick={() =>
                mergeMutation.mutate(new Date().toISOString().slice(0, 10))
              }
              disabled={mergeMutation.isLoading}
            >
              {mergeMutation.isLoading ? "Merging…" : "Merge Today"}
            </Button>
            {(usageQ.isLoading || costQ.isLoading) && (
              <span className="text-sm">Loading…</span>
            )}
            {error && <span className="text-sm text-red-600">{error}</span>}
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
