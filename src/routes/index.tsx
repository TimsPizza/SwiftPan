import { AppRoot } from "@/routes/_layout";
import { Navigate, RouteObject } from "react-router-dom";
import FilesPage from "./pages/FilesPage";
import LogPage from "./pages/LogPage";
import SettingsPage from "./pages/SettingsPage";
import TransfersPage from "./pages/TransfersPage";
import UsagePage from "./pages/UsagePage";

// Next.js-like routing structure using nested routes
// - RootLayout provides global providers and hydration gate
// - AppShell wraps authenticated app pages with sidebar layout
// - AuthShell wraps auth pages

export const routes: RouteObject[] = [
  {
    element: <AppRoot />,
    children: [
      { index: true, element: <Navigate to="/files" replace /> },
      { path: "files", element: <FilesPage /> },
      { path: "transfers", element: <TransfersPage /> },
      { path: "usage", element: <UsagePage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "logs", element: <LogPage /> },
    ],
  },
];

export default routes;
