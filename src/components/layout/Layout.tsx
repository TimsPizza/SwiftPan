"use client";

import { ReactNode } from "react";
import { Sidebar } from "./Sidebar";

export const Layout = ({ children }: { children: ReactNode }) => {
  // Show main app with layout
  return (
    <div className="bg-background flex h-screen">
      <Sidebar />
      <div className="flex min-h-0 flex-1 flex-col">
        <main className="bg-muted/20 flex-1 overflow-y-auto p-6">
          {children}
        </main>
      </div>
    </div>
  );
};
