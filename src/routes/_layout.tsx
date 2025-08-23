import { LoadingSpinner } from "@/components/fallback/LoadingSpinner";
import { AppLayout } from "@/components/layouts/AppLayout";
import { AuthLayout } from "@/components/layouts/AuthLayout";
import { useAuthStore } from "@/store/auth-store";
import "@/styles/globals.css";
import { ThemeProvider } from "next-themes";
import { HelmetProvider } from "react-helmet-async";
import { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "react-query";
import { Outlet } from "react-router-dom";
import { Toaster } from "sonner";

const queryClient = new QueryClient();

export function RootLayout({ children }: { children?: ReactNode }) {
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
          {children ?? <Outlet />}
          <Toaster richColors position="top-center" />
        </ThemeProvider>
      </HelmetProvider>
    </QueryClientProvider>
  );
}

export function AppShell() {
  return (
    <AppLayout>
      <Outlet />
    </AppLayout>
  );
}

export function AuthShell() {
  return (
    <AuthLayout>
      <Outlet />
    </AuthLayout>
  );
}
