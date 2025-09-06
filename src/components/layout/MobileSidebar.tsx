import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/utils";
import {
  BarChartIcon,
  DashboardIcon,
  FileIcon,
  GearIcon,
} from "@radix-ui/react-icons";
import { CodeIcon, MoonIcon, SunIcon } from "lucide-react";
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
    { href: "/transfers", icon: FileIcon, label: "Transfers" },
    { href: "/settings", icon: GearIcon, label: "Settings" },
    { href: "/logs", icon: CodeIcon, label: "Logs" },
  ];

  return (
    <div
      id="mobile-sidebar-container"
      className="text-foreground bg-background flex h-full w-[224px] flex-col py-3 pr-3"
    >
      <div className="mb-3 flex items-center gap-2 px-1">
        <DashboardIcon className="h-5 w-5" />
        <h2 className="text-base font-semibold tracking-tight">Menu</h2>
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-1">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = location.pathname === item.href;
          return (
            <NavLink key={item.href} to={item.href} onPointerDown={onNavigate}>
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
      <div className="mt-auto flex items-center justify-end gap-2">
        <Button
          variant="outline"
          size="icon"
          onClick={toggleTheme}
          aria-label="Toggle theme"
        >
          {theme === "dark" ? (
            <SunIcon width="20" height="20" />
          ) : (
            <MoonIcon width="20" height="20" />
          )}
        </Button>
      </div>
    </div>
  );
}

export default MobileSidebar;
