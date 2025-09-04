import EventBridge from "@/components/EventBridge";
import { LoadingSpinner } from "@/components/fallback/LoadingSpinner";
import { Sidebar } from "@/components/layout/Sidebar";
import TransferManager from "@/components/TransferManager";
import { useAuthStore } from "@/store/auth-store";
import { ThemeProvider } from "next-themes";
import { HelmetProvider } from "react-helmet-async";
import { QueryClient, QueryClientProvider } from "react-query";
import { Outlet } from "react-router-dom";
import { Toaster } from "sonner";

const queryClient = new QueryClient();

export function AppRoot() {
  const hasStoreHydrated = useAuthStore((s) => s._hydrated);

  if (!hasStoreHydrated) {
    return (
      <div className="bg-background flex min-h-screen items-center justify-center">
        <LoadingSpinner size="large" text="Initializing..." />
      </div>
    );
  }

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
