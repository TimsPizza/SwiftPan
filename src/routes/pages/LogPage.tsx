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
import { nv } from "@/lib/api/tauriBridge";
import { useEffect, useRef, useState } from "react";

export default function LogPage() {
  const [level, setLevel] = useState<
    "trace" | "debug" | "info" | "warn" | "error"
  >("info");
  const [lines, setLines] = useState<string[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const [pollMs, setPollMs] = useState(1000);
  const tailRef = useRef<HTMLDivElement>(null);

  const toggleAutoScroll = () => {
    setAutoScroll((s) => !s);
  };

  const fetchTail = async () => {
    const r = await nv.log_tail(400);
    r.match(
      (text) => setLines(String(text || "").split("\n")),
      () => {},
    );
  };

  useEffect(() => {
    let timer: any;
    const loop = async () => {
      await fetchTail();
      timer = setTimeout(loop, pollMs);
    };
    loop();
    return () => clearTimeout(timer);
  }, [pollMs]);

  useEffect(() => {
    if (autoScroll && tailRef.current) {
      tailRef.current.scrollTop = tailRef.current.scrollHeight;
    }
  }, [lines, autoScroll]);

  useEffect(() => {
    applyLevel(level);
  }, [level]);

  const applyLevel = async (level: string) => {
    // guarenteed to be one of the values
    await nv.log_set_level(level as any);
  };

  const clearLogs = async () => {
    await nv.log_clear();
    await fetchTail();
  };

  const exportLogs = () => {
    const blob = new Blob([lines.join("\n")], {
      type: "text/plain;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `swiftpan.log`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-3">
      <Card>
        <CardHeader className="flex flex-col justify-between">
          <CardTitle>Logs</CardTitle>
          <div className="flex flex-col items-center gap-2">
            <div>
              <Label>Log Level</Label>
              <Select
                value={level}
                onValueChange={(v: any) => setLevel(v as any)}
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
            </div>
            <div>
              <Label>Autoscroll</Label>
              <Switch checked={autoScroll} onCheckedChange={toggleAutoScroll} />
              <Select
                value={String(pollMs)}
                onValueChange={(v: any) => setPollMs(Number(v))}
              >
                <SelectTrigger size="sm">
                  <SelectValue placeholder="Poll" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="500">500ms</SelectItem>
                  <SelectItem value="1000">1s</SelectItem>
                  <SelectItem value="2000">2s</SelectItem>
                  <SelectItem value="5000">5s</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <Button onClick={clearLogs}>Clear</Button>
            <Button onClick={exportLogs}>Export</Button>
          </div>
        </CardHeader>
        <CardContent>
          <div
            ref={tailRef}
            className="bg-background border-muted max-h-[60vh] w-full overflow-auto rounded border p-2 font-mono text-xs"
          >
            {lines.map((ln, idx) => (
              <div key={idx} className="whitespace-pre-wrap">
                {ln}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
