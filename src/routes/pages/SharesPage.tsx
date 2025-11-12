import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { api } from "@/lib/api/tauriBridge";
import { formatRelativeTimeFuture, truncateFilename } from "@/lib/utils";
import { useCallback, useEffect, useState } from "react";

type ShareEntry = {
  key: string;
  url: string;
  created_at_ms: number;
  expires_at_ms: number;
  ttl_secs: number;
  download_filename?: string;
};

export default function SharesPage() {
  const [data, setData] = useState<ShareEntry[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const fetchShares = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.share_list();
      setData(res ?? []);
    } catch (err) {
      console.error(err);
      setData([]);
      setError(String((err as any)?.message || err || "Failed to load shares"));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchShares();
  }, [fetchShares]);

  return (
    <Card className="max-h-full flex-1 flex flex-col min-h-0">
      <CardHeader className="flex flex-row items-center justify-between gap-3">
        <CardTitle>Shares</CardTitle>
        <Button
          size="sm"
          variant="outline"
          disabled={loading}
          onClick={() => void fetchShares()}
        >
          Refresh
        </Button>
      </CardHeader>
      <CardContent className="min-h-0 max-h-full flex flex-col ">
        {loading ? (
          <div className="text-muted-foreground text-sm">Loading…</div>
        ) : error ? (
          <div className="flex flex-col gap-2">
            <div className="text-destructive text-sm">{error}</div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void fetchShares()}
            >
              Retry
            </Button>
          </div>
        ) : data.length === 0 ? (
          <div className="text-muted-foreground text-sm">No shares yet.</div>
        ) : (
          <div className="min-h-0 flex flex-col gap-4">
            <Table className="w-full text-sm overflow-auto max-h-full min-h-0">
              <TableHeader>
                <TableRow className="text-left">
                  <TableHead className="p-2">File</TableHead>
                  <TableHead className="p-2">Expires In</TableHead>
                  <TableHead className="p-2">URL</TableHead>
                  <TableHead className="p-2 text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.map((s: ShareEntry) => (
                  <TableRow
                    key={`${s.key}-${s.expires_at_ms / 1e3}`}
                    className="min-w-0 border-b whitespace-normal! last:border-0"
                  >
                    <TableCell className="min-w-0 p-2 whitespace-normal!">
                      {truncateFilename(s.key, 24)}
                    </TableCell>
                    <TableCell className="p-2">
                      {formatRelativeTimeFuture(s.expires_at_ms)}
                    </TableCell>
                    <TableCell
                      className="max-w-[420px] min-w-0 truncate p-2 whitespace-normal!"
                      title={s.url}
                    >
                      {truncateFilename(s.url, 40)}
                    </TableCell>
                    <TableCell className="p-2 text-right">
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() =>
                          void navigator.clipboard?.writeText(s.url)
                        }
                      >
                        Copy
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <div className="mt-4 px-2">
              <p className="text-muted-foreground text-sm">
                {`Once shared, the link cannot be revoked.`}
              </p>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
