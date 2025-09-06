import { LoadingSpinner } from "@/components/fallback/LoadingSpinner";
import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { nv, queries } from "@/lib/api/tauriBridge";
import { useLogStore } from "@/store/log-store";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

export default function LogPage() {
  const [level, setLevel] = useState<
    "trace" | "debug" | "info" | "warn" | "error"
  >("info");
  const entries = useLogStore((s) => s.entries);
  const [autoScroll, setAutoScroll] = useState(true);
  const tailRef = useRef<HTMLDivElement>(null);
  const { data: logStatus, isLoading } = queries.useLogStatus();
  const toggleAutoScroll = () => {
    setAutoScroll((s) => !s);
  };

  useEffect(() => {
    if (autoScroll && tailRef.current) {
      tailRef.current.scrollTop = tailRef.current.scrollHeight;
    }
  }, [entries, autoScroll]);

  useEffect(() => {
    if (logStatus?.level) {
      setLevel(String(logStatus.level).toLowerCase() as any);
    }
  }, [logStatus?.level]);

  const applyLevel = async (level: string) => {
    // guarenteed to be one of the values
    await nv.log_set_level(level as any);
  };

  const clearLogs = async () => {
    await nv.log_clear();
    useLogStore.getState().clear();
  };

  const copyLogs = () => {
    const existing = useLogStore
      .getState()
      .entries.map((e) => e.line)
      .join("\n");
    navigator.clipboard.writeText(existing);
    toast.success("Logs copied to clipboard");
  };

  return (
    <div className="space-y-3">
      <Card>
        <CardHeader className="flex flex-col justify-between">
          <CardTitle>
            <h2>Logs</h2>
          </CardTitle>
          <div className="flex flex-col items-start gap-4">
            <div className="flex items-center gap-2">
              <Label>Log Level</Label>
              {isLoading ? (
                <LoadingSpinner />
              ) : (
                <Select
                  value={level}
                  onValueChange={(v: any) => {
                    setLevel(v as any);
                    void applyLevel(v);
                  }}
                >
                  <SelectTrigger size="sm">
                    <SelectValue placeholder="Level" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="trace">Trace</SelectItem>
                    <SelectItem value="debug">Debug</SelectItem>
                    <SelectItem value="info">Info</SelectItem>
                    <SelectItem value="warn">Warn</SelectItem>
                    <SelectItem value="error">Error</SelectItem>
                  </SelectContent>
                </Select>
              )}
            </div>
            <div className="flex items-center gap-2">
              <Label>Autoscroll</Label>
              <Switch
                className="scale-110"
                checked={autoScroll}
                onCheckedChange={toggleAutoScroll}
              />
            </div>
            <div className="flex items-center gap-2">
              <Button size="sm" variant={"destructive"} onClick={clearLogs}>
                Clear
              </Button>
              <Button size="sm" onClick={copyLogs}>
                Copy
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div
            ref={tailRef}
            id="log-container"
            className="bg-background border-muted max-h-[50vh] w-full overflow-y-auto rounded-md border-2 p-2 font-mono text-xs"
          >
            {entries.map((e, idx) => (
              <div key={idx} className="py-1 whitespace-pre-wrap">
                {e.line}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
