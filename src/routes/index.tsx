import TauriBrowser from "@/components/TauriBrowser";
import { Navigate, RouteObject } from "react-router-dom";
import { AppShell, AuthShell, RootLayout } from "./_layout";
import FilesPage from "./pages/FilesPage";
import SettingsPage from "./pages/SettingsPage";
import UsagePage from "./pages/UsagePage";

// Next.js-like routing structure using nested routes
// - RootLayout provides global providers and hydration gate
// - AppShell wraps authenticated app pages with sidebar layout
// - AuthShell wraps auth pages

export const routes: RouteObject[] = [
  {
    element: <RootLayout />,
    children: [
      {
        element: <AppShell />,
        children: [
          { index: true, element: <Navigate to="/files" replace /> },
          { path: "files", element: <FilesPage /> },
          { path: "usage", element: <UsagePage /> },
          { path: "settings", element: <SettingsPage /> },
          { path: "tauri-browser", element: <TauriBrowser /> },
        ],
      },
      {
        element: <AuthShell />,
        children: [
          // { path: "login", element: <LoginPage /> },
          // { path: "setup", element: <SetupPage /> },
        ],
      },
    ],
  },
];

export default routes;
