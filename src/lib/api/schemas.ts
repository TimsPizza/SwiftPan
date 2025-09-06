import { z } from "zod";

// Minimal placeholder types/exports used across UI
export type FileItem = {
  id: string; // key
  filename: string;
  size: number;
  mimeType: string;
  uploadedAt: number; // ms
  originalName: string; // full key
};

// Settings form validation (RHF + zod)
export const R2ConfigSchema = z.object({
  endpoint: z.string().url({ message: "Endpoint must be a valid URL" }),
  access_key_id: z.string().min(1, "Access Key ID is required"),
  secret_access_key: z.string().min(1, "Secret Access Key is required"),
  bucket: z.string().min(1, "Bucket is required"),
  region: z.string().min(1, "Region is required").default("auto"),
});

export const SettingsSchema = z.object({
  endpoint: z.string().url({ message: "Endpoint must be a valid URL" }),
  access_key_id: z.string().min(1, "Access Key ID is required"),
  secret_access_key: z.string().min(1, "Secret Access Key is required"),
  bucket: z.string().min(1, "Bucket is required"),
  region: z.string().min(1, "Region is required").default("auto"),
});

export type SettingsFormValues = {
  endpoint: string;
  access_key_id: string;
  secret_access_key: string;
  bucket: string;
  region: string;
};

export const Schemas = {
  R2ConfigSchema,
  SettingsSchema,
} as const;
