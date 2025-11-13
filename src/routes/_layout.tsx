import EventBridge from "@/components/EventBridge";
import MobileSidebar from "@/components/layout/MobileSidebar";
import { Sidebar } from "@/components/layout/Sidebar";
// import TransferManager from "@/components/TransferManager";
import { Drawer, DrawerContent } from "@/components/ui/drawer";
import { MenuIcon } from "lucide-react";
import { ThemeProvider, useTheme } from "next-themes";
import { startTransition, useEffect, useState } from "react";
import { HelmetProvider } from "react-helmet-async";
import { Outlet } from "react-router-dom";
import { Toaster } from "sonner";

export function AppRoot() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const { theme, systemTheme } = useTheme();

  // Status bar inset now handled in index.html early script (persisted),
  // keeping React free of first-paint layout shifts.

  useEffect(() => {
    // Resolve var(--background) to an actual rgb/rgba color for theme-color.
    const probe = document.createElement("div");
    probe.style.position = "absolute";
    probe.style.visibility = "hidden";
    probe.style.pointerEvents = "none";
    probe.style.backgroundColor = "var(--background)";
    document.body.appendChild(probe);
    const bg = getComputedStyle(probe).backgroundColor || "#000000";
    probe.remove();

    let meta = document.querySelector('meta[name="theme-color"]');
    if (!meta) {
      meta = document.createElement("meta");
      meta.setAttribute("name", "theme-color");
      document.head.appendChild(meta);
    }
    meta.setAttribute("content", bg.trim());
  }, [theme, systemTheme]);
  return (
    <HelmetProvider>
      <ThemeProvider attribute="class" defaultTheme="system" enableSystem>
        <div className="bg-background app-viewport flex w-full overflow-hidden">
          {/* Sidebar: hidden on small screens, visible from md up */}
          <div className="hidden md:!block">
            <Sidebar />
          </div>
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            {/* Mobile header */}
            <div className="bg-background relative flex w-full items-center justify-between border-b px-4 py-2 md:hidden">
              <button
                aria-label="Open sidebar"
                className="hover:bg-muted absolute rounded p-2"
                onPointerDown={() =>
                  startTransition(() => {
                    setMobileOpen(true);
                  })
                }
              >
                <MenuIcon />
              </button>
              <div className="m-auto">
                <span className="text-xl font-semibold">SwiftPan</span>
              </div>
              <div />
            </div>
            <main className="bg-muted/20 min-h-0 flex-1 overflow-y-auto p-4 md:p-6">
              <Outlet />
            </main>
          </div>
        </div>
        {/* Mobile sidebar drawer */}
        <Drawer open={mobileOpen} onOpenChange={setMobileOpen}>
          <DrawerContent
            forceMount
            showCloseButton={false}
            side="left"
            className="w-auto transition-[transform]"
          >
            <MobileSidebar onNavigate={() => setMobileOpen(false)} />
          </DrawerContent>
        </Drawer>
        {/* Event Listeners */}
        <EventBridge />
        {/* Sonner Toast Container */}
        <Toaster
          richColors
          style={{ marginTop: "var(--resolved-safe-top)" }}
          position="top-center"
        />
      </ThemeProvider>
    </HelmetProvider>
  );
}
