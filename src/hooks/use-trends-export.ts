import type { TrendPoint } from "@/hooks/use-trends";

function downloadCsv(filename: string, rows: string[]) {
  const blob = new Blob([rows.join("\n")], { type: "text/csv;charset=utf-8;" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export function useUsageTrendsCsv(points: TrendPoint[]) {
  const exportCsv = (periodLabel: string) => {
    const rows = [
      "date,storage_mb,classA_ops,classB_ops",
      ...(points || []).map((p) => {
        const storageMB = Math.max(
          0,
          Math.round((p.peakStorageBytes ?? 0) / 1024 / 1024),
        );
        const classA = p.classACount ?? 0;
        const classB = p.classBCount ?? 0;
        return `${p.date},${storageMB},${classA},${classB}`;
      }),
    ];
    downloadCsv(`usage_trends_${periodLabel}.csv`, rows);
  };
  return { exportCsv };
}

export function useTrafficTrendsCsv(points: TrendPoint[]) {
  const exportCsv = (periodLabel: string) => {
    const rows = [
      "date,upload_mb,download_mb",
      ...(points || []).map((p) => {
        const uploadMB = Math.max(
          0,
          Math.round((p.uploadBytes ?? 0) / 1024 / 1024),
        );
        const downloadMB = Math.max(
          0,
          Math.round((p.downloadBytes ?? 0) / 1024 / 1024),
        );
        return `${p.date},${uploadMB},${downloadMB}`;
      }),
    ];
    downloadCsv(`traffic_trends_${periodLabel}.csv`, rows);
  };
  return { exportCsv };
}

export function useCostTrendsCsv(points: TrendPoint[]) {
  const exportCsv = (periodLabel: string) => {
    const rows = [
      "date,cost_usd",
      ...(points || []).map((p) => `${p.date},${(p.cost ?? 0).toFixed(6)}`),
    ];
    downloadCsv(`cost_trends_${periodLabel}.csv`, rows);
  };
  return { exportCsv };
}
