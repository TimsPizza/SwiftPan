import EventBridge from "@/components/EventBridge";
import MobileSidebar from "@/components/layout/MobileSidebar";
import { Sidebar } from "@/components/layout/Sidebar";
import TransferManager from "@/components/TransferManager";
import { Drawer, DrawerContent } from "@/components/ui/drawer";
import { MenuIcon } from "lucide-react";
import { ThemeProvider } from "next-themes";
import { useState } from "react";
import { HelmetProvider } from "react-helmet-async";
import { QueryClient, QueryClientProvider } from "react-query";
import { Outlet } from "react-router-dom";
import { Toaster } from "sonner";

const queryClient = new QueryClient();

export function AppRoot() {
  const [mobileOpen, setMobileOpen] = useState(false);
  return (
    <QueryClientProvider client={queryClient}>
      <HelmetProvider>
        <ThemeProvider attribute="class" defaultTheme="system" enableSystem>
          <div className="bg-background flex h-screen w-full">
            {/* Sidebar: hidden on small screens, visible from md up */}
            <div className="hidden md:!block">
              <Sidebar />
            </div>
            <div className="flex min-h-0 min-w-0 flex-1 flex-col">
              {/* Mobile navbar */}
              <div className="relative flex w-full items-center justify-between border-b px-4 py-2 md:hidden">
                <button
                  aria-label="Open sidebar"
                  className="hover:bg-muted rounded p-2"
                  onClick={() => setMobileOpen(true)}
                >
                  <MenuIcon />
                </button>
                <div className="absolute top-1/2 right-1/2 mx-auto translate-x-1/2 -translate-y-1/2 text-sm font-semibold">
                  SwiftPan
                </div>
                <div />
              </div>
              <main className="bg-muted/20 flex-1 overflow-y-auto p-4 md:p-6">
                <Outlet />
              </main>
            </div>
          </div>
          {/* Mobile sidebar drawer */}
          <Drawer open={mobileOpen} onOpenChange={setMobileOpen}>
            <DrawerContent side="left" className="w-auto">
              <MobileSidebar onNavigate={() => setMobileOpen(false)} />
            </DrawerContent>
          </Drawer>
          {/* Event Listeners */}
          <EventBridge />
          <TransferManager />
          {/* Sonner Toast Container */}
          <Toaster richColors position="top-center" />
        </ThemeProvider>
      </HelmetProvider>
    </QueryClientProvider>
  );
}
