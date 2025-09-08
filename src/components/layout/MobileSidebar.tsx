import { Button } from "@/components/ui/Button";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import {
  BarChartIcon,
  DashboardIcon,
  FileIcon,
  GearIcon,
} from "@radix-ui/react-icons";
import { ClipboardClockIcon, CodeIcon, Share2 } from "lucide-react";
import { useTheme } from "next-themes";
import { NavLink, useLocation } from "react-router-dom";

export function MobileSidebar({ onNavigate }: { onNavigate?: () => void }) {
  const location = useLocation();
  const { theme, setTheme } = useTheme();
  const toggleTheme = () => {
    const next = theme === "dark" ? "light" : "dark";
    setTheme(next);
    localStorage.setItem("theme", next);
  };
  const navItems = [
    { href: "/files", icon: FileIcon, label: "Files" },
    { href: "/usage", icon: BarChartIcon, label: "Usage" },
    { href: "/transfers", icon: ClipboardClockIcon, label: "Tasks" },
    { href: "/shares", icon: Share2, label: "Shares" },
    { href: "/settings", icon: GearIcon, label: "Settings" },
    { href: "/logs", icon: CodeIcon, label: "Logs" },
  ];

  return (
    <div
      id="mobile-sidebar-container"
  className="text-foreground border-sidebar-border bg-background flex h-full w-[224px] flex-col pr-3"
  style={{ paddingTop: "var(--resolved-safe-top)", paddingBottom: "0.75rem" }}
    >
      <div className="flex items-center gap-3 px-2">
        <DashboardIcon className="h-5 w-5" />
        <span className="text-lg font-semibold tracking-tight">SwiftPan</span>
      </div>
      <Separator className="my-4" />
      <div className="flex min-h-0 flex-col gap-1">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = location.pathname === item.href;
          return (
            <NavLink
              key={item.href}
              to={item.href}
              onClick={() => {
                // Close drawer after navigation is initiated; avoid pointerdown preempting link routing
                onNavigate?.();
              }}
            >
              <Button
                variant={"ghost"}
                className={cn(
                  "h-11 w-full cursor-pointer justify-start gap-3 rounded-md",
                  isActive && "bg-primary/15 hover:bg-primary/20 text-primary",
                )}
              >
                <Icon className="h-5 w-5" />
                <span
                  className={cn(
                    "text-sm",
                    isActive ? "font-semibold" : "font-medium",
                  )}
                >
                  {item.label}
                </span>
              </Button>
            </NavLink>
          );
        })}
      </div>
      <Separator className="my-4" />
      <div className="flex items-center gap-2 px-3">
        <Label htmlFor="theme-toggle">Dark Mode</Label>
        <Switch
          id="theme-toggle"
          checked={theme === "dark"}
          onCheckedChange={toggleTheme}
        />
      </div>
    </div>
  );
}

export default MobileSidebar;
