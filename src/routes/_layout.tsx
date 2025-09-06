import EventBridge from "@/components/EventBridge";
import MobileSidebar from "@/components/layout/MobileSidebar";
import { Sidebar } from "@/components/layout/Sidebar";
// import TransferManager from "@/components/TransferManager";
import { Drawer, DrawerContent } from "@/components/ui/drawer";
import { MenuIcon } from "lucide-react";
import { ThemeProvider, useTheme } from "next-themes";
import { useEffect, useState } from "react";
import { HelmetProvider } from "react-helmet-async";
import { QueryClient, QueryClientProvider } from "react-query";
import { Outlet } from "react-router-dom";
import { Toaster } from "sonner";

const queryClient = new QueryClient();

export function AppRoot() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const { theme, systemTheme } = useTheme();

  // Auto-detect a reasonable top inset; fallback for Android overlay status bar.
  useEffect(() => {
    const ua = navigator.userAgent || "";
    const isAndroid = /Android/i.test(ua);

    const applyFallback = () => {
      const vv: any = (window as any).visualViewport;
      let top = 0;
      if (vv) {
        top = Math.max(0, Math.round(vv.offsetTop || 0));
        if (top === 0) {
          const diff = Math.max(0, Math.round(window.innerHeight - vv.height));
          if (diff > 0) top = Math.min(28, Math.max(0, 24));
        }
      } else if (isAndroid) {
        top = 24;
      }
      document.documentElement.style.setProperty(
        "--fallback-top",
        `${top}px`,
      );
    };

    applyFallback();
    const vv: any = (window as any).visualViewport;
    vv?.addEventListener?.("resize", applyFallback);
    window.addEventListener("orientationchange", applyFallback);
    return () => {
      vv?.removeEventListener?.("resize", applyFallback);
      window.removeEventListener("orientationchange", applyFallback);
    };
  }, []);

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
    <QueryClientProvider client={queryClient}>
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
                  onPointerDown={() => setMobileOpen(true)}
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
          <Toaster richColors position="top-center" />
        </ThemeProvider>
      </HelmetProvider>
    </QueryClientProvider>
  );
}
