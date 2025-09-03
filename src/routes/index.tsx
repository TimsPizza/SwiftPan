import { RouteObject } from "react-router-dom";
import { RootLayout, AppShell, AuthShell } from "./_layout";
import App from "@/App";
import TauriBrowser from "@/components/TauriBrowser";

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
          { index: true, element: <App /> },
          { path: "tauri-browser", element: <TauriBrowser /> },
          // { path: "files", element: <FilesPage /> },
          // { path: "usage", element: <UsagePage /> },
          // { path: "settings", element: <SettingsPage /> },
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
