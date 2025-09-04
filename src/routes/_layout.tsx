import EventBridge from "@/components/EventBridge";
import { Sidebar } from "@/components/layout/Sidebar";
import TransferManager from "@/components/TransferManager";
import { ThemeProvider } from "next-themes";
import { HelmetProvider } from "react-helmet-async";
import { QueryClient, QueryClientProvider } from "react-query";
import { Outlet } from "react-router-dom";
import { Toaster } from "sonner";

const queryClient = new QueryClient();

export function AppRoot() {
  return (
    <QueryClientProvider client={queryClient}>
      <HelmetProvider>
        <ThemeProvider attribute="class" defaultTheme="system" enableSystem>
          <div className="bg-background flex h-screen">
            <Sidebar />
            <div className="flex min-h-0 flex-1 flex-col">
              <main className="bg-muted/20 flex-1 overflow-y-auto p-6">
                <Outlet />
              </main>
            </div>
          </div>
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
