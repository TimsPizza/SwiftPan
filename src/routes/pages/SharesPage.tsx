import { Button } from "@/components/ui/Button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { nv } from "@/lib/api/tauriBridge";
import { formatRelativeTimeFuture, truncateFilename } from "@/lib/utils";
import { useEffect, useState } from "react";

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
  useEffect(() => {
    let alive = true;
    (async () => {
      setLoading(true);
      try {
        const res = await nv.share_list();
        res.match(
          (v: ShareEntry[]) => alive && setData(v),
          () => {},
        );
      } finally {
        if (alive) setLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="space-y-4">
      <h2 className="text-xl font-semibold">Shares</h2>
      {loading ? (
        <div className="text-muted-foreground text-sm">Loading…</div>
      ) : data.length === 0 ? (
        <div className="text-muted-foreground text-sm">No shares yet.</div>
      ) : (
        <div className="overflow-x-auto">
          <Table className="w-full text-sm">
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
                      onClick={() => void navigator.clipboard?.writeText(s.url)}
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
    </div>
  );
}
