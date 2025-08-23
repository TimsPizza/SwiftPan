import type {
  GetQuotaStatusResponse,
  GetQuotaUsageResponse,
} from "@/lib/api/schemas";

export interface UsageSummary {
  // storage
  storageSegments: Array<{
    label: string;
    max: number;
    current: number;
    color?: "primary" | "secondary" | "success" | "warning" | "danger";
  }>;

  // class ops
  classASegments: Array<{
    label: string;
    max: number;
    current: number;
    color?: "primary" | "secondary" | "success" | "warning" | "danger";
  }>;
  classBSegments: Array<{
    label: string;
    max: number;
    current: number;
    color?: "primary" | "secondary" | "success" | "warning" | "danger";
  }>;

  // budget
  costUsed: number;
  costLimit: number;
  budgetPct: number;
  budgetColor: "primary" | "warning" | "danger";

  daysLeftInMonth: number;
}

function computeDaysLeftInMonth(): number {
  const now = new Date();
  const end = new Date(now.getFullYear(), now.getMonth() + 1, 0);
  return Math.max(
    0,
    Math.ceil((end.getTime() - now.getTime()) / (24 * 60 * 60 * 1000)),
  );
}

function getSemanticColor(pct: number): "primary" | "warning" | "danger" {
  if (pct >= 100) return "danger";
  if (pct >= 80) return "warning";
  return "primary";
}

export function useUsageSummary(
  data:
    | {
        usage: GetQuotaUsageResponse | null;
        status: GetQuotaStatusResponse | null;
      }
    | null
    | undefined,
): UsageSummary {
  const usage = data?.usage ?? null;
  const status = data?.status ?? null;

  // Storage
  const storageBytes = usage?.storageBytes ?? 0;
  const freeStorageBytes = usage?.freeTier?.storageBytes ?? 0;
  const storageLimitGB = usage?.quota?.storageLimitGB ?? 0;
  const storageLimitBytes = Math.max(
    0,
    Math.floor(storageLimitGB * 1024 * 1024 * 1024),
  );
  const storageBudgetCap = Math.max(0, storageLimitBytes - freeStorageBytes);

  const storageFreeUsed = Math.min(storageBytes, freeStorageBytes);
  const storageBudgetUsed = Math.min(
    Math.max(storageBytes - freeStorageBytes, 0),
    storageBudgetCap,
  );

  const storageSegments = [
    {
      label: "Free Tier",
      max: freeStorageBytes,
      current: storageFreeUsed,
      color: "primary" as const,
    },
    ...(storageBudgetCap > 0
      ? [
          {
            label: "Budget",
            max: storageBudgetCap,
            current: storageBudgetUsed,
            color: "secondary" as const,
          },
        ]
      : []),
  ];

  // Class A
  const classAUsed = usage?.operationsCount?.classA ?? 0;
  const classAFree = usage?.freeTier?.classAOperations ?? 0;
  const classALimitM = usage?.quota?.classALimitM ?? 0;
  const classALimitOps = Math.max(0, Math.floor(classALimitM * 1_000_000));
  const classABudgetCap = Math.max(0, classALimitOps - classAFree);

  const classAFreeUsed = Math.min(classAUsed, classAFree);
  const classABudgetUsed = Math.min(
    Math.max(classAUsed - classAFree, 0),
    classABudgetCap,
  );

  const classASegments = [
    {
      label: "Free Tier",
      max: classAFree,
      current: classAFreeUsed,
      color: "primary" as const,
    },
    ...(classABudgetCap > 0
      ? [
          {
            label: "Budget",
            max: classABudgetCap,
            current: classABudgetUsed,
            color: "secondary" as const,
          },
        ]
      : []),
  ];

  // Class B
  const classBUsed = usage?.operationsCount?.classB ?? 0;
  const classBFree = usage?.freeTier?.classBOperations ?? 0;
  const classBLimitM = usage?.quota?.classBLimitM ?? 0;
  const classBLimitOps = Math.max(0, Math.floor(classBLimitM * 1_000_000));
  const classBBudgetCap = Math.max(0, classBLimitOps - classBFree);

  const classBFreeUsed = Math.min(classBUsed, classBFree);
  const classBBudgetUsed = Math.min(
    Math.max(classBUsed - classBFree, 0),
    classBBudgetCap,
  );

  const classBSegments = [
    {
      label: "Free Tier",
      max: classBFree,
      current: classBFreeUsed,
      color: "primary" as const,
    },
    ...(classBBudgetCap > 0
      ? [
          {
            label: "Budget",
            max: classBBudgetCap,
            current: classBBudgetUsed,
            color: "secondary" as const,
          },
        ]
      : []),
  ];

  // Budget
  const costUsed = usage?.currentMonthlyCost ?? 0;
  const costLimit = usage?.monthlyBudget ?? status?.monthlyBudget ?? 0;
  const budgetPct = costLimit > 0 ? (costUsed / costLimit) * 100 : 0;
  const budgetColor = getSemanticColor(budgetPct);

  const daysLeftInMonth = computeDaysLeftInMonth();

  return {
    storageSegments,
    classASegments,
    classBSegments,
    costUsed,
    costLimit,
    budgetPct,
    budgetColor,
    daysLeftInMonth,
  };
}
