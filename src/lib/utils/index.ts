import clsx, { ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * A utility function to merge Tailwind CSS classes with clsx.
 * It safely merges conflicting classes.
 * @param inputs - The class values to merge.
 * @returns The merged class string.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes)) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let num = bytes;
  while (num >= 1024 && i < units.length - 1) {
    num /= 1024;
    i++;
  }
  return `${num.toFixed(num < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

export function truncateFilename(name: string, max = 24): string {
  if (name.length <= max) return name;
  const half = Math.max(3, Math.floor((max - 3) / 2));
  return `${name.slice(0, half)}...${name.slice(-half)}`;
}

export function formatRelativeTime(ts: number | Date): string {
  const now = Date.now();
  const t = ts instanceof Date ? ts.getTime() : ts;
  const diff = Math.floor((now - t) / 1000);
  if (diff < 60) return `${diff}s ago`;
  const m = Math.floor(diff / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  return `${d}d ago`;
}
