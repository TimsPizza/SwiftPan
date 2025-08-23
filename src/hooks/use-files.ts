import { fileApi } from "@/lib/api";
import { useQuery } from "react-query";

export const useFiles = () => {
  return useQuery("files", () => fileApi.getFiles(), {
    // Optional: configure refetch intervals, stale times, etc.
    // refetchInterval: 5000, // e.g., refetch every 5 seconds
  });
};
