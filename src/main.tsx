import { applyStatusBarInsetFromNative } from "@/lib/api/tauriBridge";
import "@/styles/globals.css";
import "@/styles/tailwind.css";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider, createBrowserRouter } from "react-router-dom";
import { routes } from "./routes";
const router = createBrowserRouter(routes);
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 0, refetchOnWindowFocus: false },
    mutations: { retry: 0 },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </React.StrictMode>,
);

// Kick native inset fetch post-hydration to refine value and persist
applyStatusBarInsetFromNative();
