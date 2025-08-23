"use client";

import { Layout } from "@/components/layout/Layout";
import { ReactNode } from "react";

export const AppLayout = ({ children }: { children: ReactNode }) => {
  return <Layout>{children}</Layout>;
};
