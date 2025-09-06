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

export const Sidebar = () => {
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
    { href: "/settings", icon: GearIcon, label: "Settings" },
    { href: "/logs", icon: CodeIcon, label: "Logs" },
  ];

  return (
    <div
      id="sidebar-container"
      className="bg-sidebar text-sidebar-foreground border-sidebar-border flex h-full w-56 flex-col border-r p-4"
    >
      <div className="flex min-h-0 flex-1 flex-col gap-2">
        <div className="mb-4 flex items-center gap-2 px-2">
          <DashboardIcon className="text-sidebar-primary h-5 w-5" />
          <h2 className="text-lg font-semibold tracking-tight">Navigation</h2>
        </div>

        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = location.pathname === item.href;

          return (
            <NavLink key={item.href} to={item.href}>
              <Button
                variant={"ghost"}
                className={cn(
                  "text-sidebar-foreground h-12 w-full cursor-pointer justify-start gap-3",
                  isActive &&
                    "bg-sidebar-primary/40 hover:!bg-sidebar-primary/20 text-sidebar-secondary",
                )}
              >
                <Icon className="h-5 w-5" />
                <span
                  className={cn(
                    "text-sm",
                    isActive ? "font-bold" : "font-medium",
                  )}
                >
                  {item.label}
                </span>
              </Button>
            </NavLink>
          );
        })}
      </div>
      <div className="mt-auto ml-auto flex items-center gap-2">
        <Button
          variant="outline"
          size="icon"
          onClick={toggleTheme}
          aria-label="Toggle theme"
        >
          {theme === "dark" ? (
            <SunIcon width="24" height="24" />
          ) : (
            <MoonIcon width="24" height="24" />
          )}
        </Button>
      </div>
    </div>
  );
};
