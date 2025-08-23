"use client";

import { TrafficTrendsChart } from "@/components/charts/TrafficTrendsChart";
import { UsageTrendsChart } from "@/components/charts/UsageTrendsChart";
import { ErrorDisplay } from "@/components/fallback/ErrorDisplay";
import { LoadingSpinner } from "@/components/fallback/LoadingSpinner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/Button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ProgressCard } from "@/components/ui/ProgressCard";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useQuotaData } from "@/hooks/use-quota-data";
import { useTrafficTrends, useUsageTrends } from "@/hooks/use-trends";
import {
  useTrafficTrendsCsv,
  useUsageTrendsCsv,
} from "@/hooks/use-trends-export";
import { useUsageSummary } from "@/hooks/use-usage-summary";
import { formatRelativeTime } from "@/lib/utils";
import { CheckCircledIcon } from "@radix-ui/react-icons";
import { BarChart3, RefreshCw } from "lucide-react";
import { Helmet } from "react-helmet-async";
import { useEffect, useMemo, useState } from "react";

export default function UsagePage() {
  const [selectedPeriod, setSelectedPeriod] = useState("7d");
  const {
    data: quotaData,
    loading,
    error,
    refetch,
    lastUpdated,
  } = useQuotaData();
  const [nowTick, setNowTick] = useState<number>(Date.now());

  useEffect(() => {
    const t = setInterval(() => setNowTick(Date.now()), 60000);
    return () => clearInterval(t);
  }, []);
  const lastUpdatedLabel = useMemo(() => {
    return lastUpdated ? formatRelativeTime(lastUpdated) : "--";
  }, [lastUpdated, nowTick]);
  const usageTrends = useUsageTrends(7);
  const trafficTrends = useTrafficTrends(7);
  const usageCsv = useUsageTrendsCsv(usageTrends.trends);
  const trafficCsv = useTrafficTrendsCsv(trafficTrends.trends);

  // Period mapping for trends refetch
  const periodToDays: Record<string, number> = {
    "24h": 1,
    "7d": 7,
    "14d": 14,
    "30d": 30,
    "60d": 60,
    "90d": 90,
  };

  useEffect(() => {
    const days = periodToDays[selectedPeriod] ?? 7;
    // keep quota usage in sync
    refetch(days);
    // drive dedicated trends hooks
    usageTrends.setDays(days);
    trafficTrends.setDays(days);
  }, [selectedPeriod]);

  // Summaries via hook to simplify page
  const summary = useUsageSummary(quotaData);

  // Trends panel state
  const [showSeries, setShowSeries] = useState<{
    storage: boolean;
    classA: boolean;
    classB: boolean;
  }>({ storage: true, classA: true, classB: true });
  // CSV using hooks
  const exportUsageCsv = () => usageCsv.exportCsv(selectedPeriod);
  const exportTrafficCsv = () => trafficCsv.exportCsv(selectedPeriod);

  if (loading) {
    return (
      <div className="container mx-auto p-6">
        <LoadingSpinner size="large" text="Loading usage data..." />
      </div>
    );
  }

  if (error) {
    return (
      <div className="container mx-auto p-6">
        <ErrorDisplay error={error} onRetry={() => refetch()} />
      </div>
    );
  }

  return (
    <>
      <Helmet>
        <title>Usage Analytics - R2Vault</title>
        <meta
          name="description"
          content="View your cloudflare r2 usage and costs"
        />
      </Helmet>

      <div className="container mx-auto space-y-6 p-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="flex items-center gap-3 text-3xl font-bold tracking-tight">
              <BarChart3 className="text-primary h-8 w-8" />
              Usage Analytics
            </h1>
            <p className="text-muted-foreground mt-1">
              Monitor your storage usage and costs
            </p>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-muted-foreground text-sm">
              Last updated: {lastUpdatedLabel}
            </span>
            <Button variant="outline" size="sm" onClick={() => refetch()}>
              <RefreshCw className="mr-2 h-4 w-4" />
              Refresh
            </Button>
          </div>
        </div>
        {/* Monthly Usage Card */}
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <CheckCircledIcon className="text-primary h-5 w-5" />
                System Health
              </CardTitle>
              <CardDescription>
                Current system status and performance
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">Storage System</span>
                <Badge className="bg-primary/20 text-primary-foreground">
                  Healthy
                </Badge>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">Cost Monitoring</span>
                <Badge className="bg-primary/20 text-primary-foreground">
                  Active
                </Badge>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">Backup Status</span>
                <Badge className="bg-primary/20 text-primary-foreground">
                  Synced
                </Badge>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">Last Updated</span>
                <span className="text-muted-foreground text-sm">
                  {lastUpdatedLabel}
                </span>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-3">
              <div className="flex items-center justify-between">
                <CardTitle className="text-base">Monthly Quota Used</CardTitle>
                <Badge variant="secondary" className="text-xs">
                  Reset in {summary.daysLeftInMonth} day(s)
                </Badge>
              </div>
            </CardHeader>
            <CardContent className="grid grid-cols-1 gap-4 lg:grid-cols-2">
              {/* Storage row */}
              <ProgressCard
                title="Storage"
                unit="bytes"
                showPercentage={true}
                segments={summary.storageSegments}
              />

              {/* Class A row */}
              <ProgressCard
                title="Class A"
                unit="ops"
                showPercentage={true}
                segments={summary.classASegments}
              />

              {/* Class B row */}
              <ProgressCard
                title="Class B"
                unit="ops"
                showPercentage={true}
                segments={summary.classBSegments}
              />

              {/* Budget row */}
              <ProgressCard
                title="Budget"
                current={summary.costUsed}
                total={Math.max(0, summary.costLimit)}
                unit="USD"
                color={summary.budgetColor}
                showPercentage={summary.budgetPct > 0}
              />
            </CardContent>
          </Card>

          {/* Usage Trends Panel */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-base">Usage Trends</CardTitle>

              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Select
                    value={selectedPeriod}
                    onValueChange={setSelectedPeriod}
                  >
                    <SelectTrigger className="w-28">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="7d">7 Days</SelectItem>
                      <SelectItem value="14d">14 Days</SelectItem>
                      <SelectItem value="30d">30 Days</SelectItem>
                      <SelectItem value="90d">90 Days</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex items-center gap-2">
                  <Button variant="outline" size="sm" onClick={exportUsageCsv}>
                    Export CSV
                  </Button>
                </div>
              </div>
              <div className="flex items-center gap-2 pt-2">
                <Button
                  variant={showSeries.storage ? "default" : "outline"}
                  size="sm"
                  onClick={() =>
                    setShowSeries((s) => ({ ...s, storage: !s.storage }))
                  }
                >
                  Storage
                </Button>
                <Button
                  variant={showSeries.classA ? "default" : "outline"}
                  size="sm"
                  onClick={() =>
                    setShowSeries((s) => ({ ...s, classA: !s.classA }))
                  }
                >
                  Class A
                </Button>
                <Button
                  variant={showSeries.classB ? "default" : "outline"}
                  size="sm"
                  onClick={() =>
                    setShowSeries((s) => ({ ...s, classB: !s.classB }))
                  }
                >
                  Class B
                </Button>
              </div>
            </CardHeader>
            <CardContent>
              <div className="h-64 w-full">
                <UsageTrendsChart
                  points={usageTrends.trends}
                  showStorage={showSeries.storage}
                  showClassA={showSeries.classA}
                  showClassB={showSeries.classB}
                />
              </div>
            </CardContent>
          </Card>
          {/* Traffic Trends Panel */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-base">Traffic Trends</CardTitle>

              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Select
                    value={selectedPeriod}
                    onValueChange={setSelectedPeriod}
                  >
                    <SelectTrigger className="w-28">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="7d">7 Days</SelectItem>
                      <SelectItem value="14d">14 Days</SelectItem>
                      <SelectItem value="30d">30 Days</SelectItem>
                      <SelectItem value="60d">60 Days</SelectItem>
                      <SelectItem value="90d">90 Days</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={exportTrafficCsv}
                  >
                    Export CSV
                  </Button>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <div className="h-64 w-full">
                <TrafficTrendsChart points={trafficTrends.trends} />
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
