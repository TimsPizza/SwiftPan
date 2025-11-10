import { TrafficTrendsChart } from "@/components/charts/TrafficTrendsChart";
import { UsageTrendsChart } from "@/components/charts/UsageTrendsChart";
import GlobalError from "@/components/fallback/GlobalError";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import {
  Table,
  TableBody,
  TableCell,
  TableFooter,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { mutations, queries } from "@/lib/api/tauriBridge";
import { formatBytes } from "@/lib/utils";
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
    staleTime: 300_000,
    cacheTime: 600_000,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
  const costQ = queries.useUsageMonthCost(month, {
    onSuccess: (ok) => setCost(ok),
    onError: (e: any) => setError(String(e?.message || e)),
    staleTime: 300_000,
    cacheTime: 600_000,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
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
            {(usageQ.isLoading || costQ.isLoading) && (
              <div className="flex items-center gap-2">
                <div className="bg-muted h-4 w-32 animate-pulse rounded" />
                <div className="bg-muted h-4 w-24 animate-pulse rounded" />
              </div>
            )}
            {error && <span className="text-sm text-red-600">{error}</span>}
          </div>
          {cost ? (
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
          ) : (
            <div className="grid grid-cols-1 gap-4">
              <div className="bg-muted h-20 w-full animate-pulse rounded" />
              <div className="bg-muted h-20 w-full animate-pulse rounded" />
              <div className="bg-muted h-20 w-full animate-pulse rounded" />
            </div>
          )}
        </CardContent>
      </Card>

      {/* Monthly Usage Table */}
      <Card>
        <CardHeader>
          <CardTitle>Usage (Table)</CardTitle>
        </CardHeader>
        <CardContent className="max-h-52 overflow-y-auto">
          {usageQ.isLoading ? (
            <div className="space-y-2">
              <div className="bg-muted h-6 w-full animate-pulse rounded" />
              <div className="bg-muted h-6 w-full animate-pulse rounded" />
              <div className="bg-muted h-6 w-full animate-pulse rounded" />
            </div>
          ) : items && items.length > 0 ? (
            <Table>
              <TableHeader className="bg-background sticky top-0">
                <TableRow>
                  <TableHead className="w-28">Date</TableHead>
                  <TableHead>Upload</TableHead>
                  <TableHead>Download</TableHead>
                  <TableHead className="hidden lg:table-cell">
                    Storage
                  </TableHead>
                  <TableHead className="hidden lg:table-cell">Peak</TableHead>
                  <TableHead className="hidden lg:table-cell">
                    Deleted
                  </TableHead>
                  <TableHead className="text-center">Class A</TableHead>
                  <TableHead className="text-center">Class B</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {[...items]
                  .sort((a: any, b: any) =>
                    String(a.date).localeCompare(String(b.date)),
                  )
                  .map((d: any) => {
                    const aOps = Object.values(d.class_a || {}).reduce(
                      (acc: number, v: any) => acc + Number(v || 0),
                      0,
                    );
                    const bOps = Object.values(d.class_b || {}).reduce(
                      (acc: number, v: any) => acc + Number(v || 0),
                      0,
                    );
                    return (
                      <TableRow key={d.date}>
                        <TableCell className="font-mono text-xs">
                          {String(d.date)}
                        </TableCell>
                        <TableCell>
                          {formatBytes(Number(d.ingress_bytes || 0))}
                        </TableCell>
                        <TableCell>
                          {formatBytes(Number(d.egress_bytes || 0))}
                        </TableCell>
                        <TableCell className="hidden lg:table-cell">
                          {formatBytes(Number(d.storage_bytes || 0))}
                        </TableCell>
                        <TableCell className="hidden lg:table-cell">
                          {formatBytes(Number(d.peak_storage_bytes || 0))}
                        </TableCell>
                        <TableCell className="hidden lg:table-cell">
                          {formatBytes(Number(d.deleted_storage_bytes || 0))}
                        </TableCell>
                        <TableCell className="text-center">
                          {aOps.toLocaleString()}
                        </TableCell>
                        <TableCell className="text-center">
                          {bOps.toLocaleString()}
                        </TableCell>
                      </TableRow>
                    );
                  })}
              </TableBody>
              <TableFooter>
                <TableRow>
                  <TableCell className="font-medium">Totals</TableCell>
                  <TableCell className="font-medium">
                    {formatBytes(
                      items.reduce(
                        (acc: number, d: any) =>
                          acc + Number(d.ingress_bytes || 0),
                        0,
                      ),
                    )}
                  </TableCell>
                  <TableCell className="font-medium">
                    {formatBytes(
                      items.reduce(
                        (acc: number, d: any) =>
                          acc + Number(d.egress_bytes || 0),
                        0,
                      ),
                    )}
                  </TableCell>
                  <TableCell className="hidden lg:table-cell" />
                  <TableCell className="hidden lg:table-cell" />
                  <TableCell className="hidden lg:table-cell" />
                  <TableCell className="text-center font-medium">
                    {items
                      .reduce(
                        (acc: number, d: any) =>
                          acc +
                          Object.values(d.class_a || {}).reduce(
                            (a: number, v: any) => a + Number(v || 0),
                            0,
                          ),
                        0,
                      )
                      .toLocaleString()}
                  </TableCell>
                  <TableCell className="font-medium text-center">
                    {items
                      .reduce(
                        (acc: number, d: any) =>
                          acc +
                          Object.values(d.class_b || {}).reduce(
                            (a: number, v: any) => a + Number(v || 0),
                            0,
                          ),
                        0,
                      )
                      .toLocaleString()}
                  </TableCell>
                </TableRow>
              </TableFooter>
            </Table>
          ) : (
            <div className="text-muted-foreground text-sm">
              No data for this month.
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
          {usageQ.isLoading ? (
            <div className="bg-muted h-64 w-full animate-pulse rounded" />
          ) : (
            <div className="h-64 w-full">
              <UsageTrendsChart points={items} />
            </div>
          )}
        </CardContent>
      </Card>

      {/* Monthly Traffic Chart (from backend daily ledgers) */}
      <Card>
        <CardHeader>
          <CardTitle>Traffic</CardTitle>
        </CardHeader>
        <CardContent>
          {usageQ.isLoading ? (
            <div className="bg-muted h-64 w-full animate-pulse rounded" />
          ) : (
            <div className="h-64 w-full">
              <TrafficTrendsChart points={trafficPoints as any} />
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
