"use client";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/Button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { useSetup } from "@/hooks/use-setup";
import { AppError } from "@/lib/api/errors";
import { settingsService } from "@/lib/settings/service";
import {
  AlertCircle,
  ArrowRight,
  CheckCircle,
  DollarSign,
  Key,
  Rocket,
  Server,
} from "lucide-react";
import { Helmet } from "react-helmet-async";
import { useNavigate } from "react-router-dom";
import { useState } from "react";
import { toast } from "sonner";

const SETUP_STEPS = [
  { id: 1, title: "Admin Password", description: "Set your admin password" },
  { id: 2, title: "R2 Configuration", description: "Configure Cloudflare R2" },
  { id: 3, title: "Budget Settings", description: "Set spending limits" },
];

export default function SetupPage() {
  const navigate = useNavigate();
  const { status: setupStatus, initializeSystem } = useSetup();
  const [currentStep, setCurrentStep] = useState(1);
  const [isProcessing, setIsProcessing] = useState(false);

  // Form states
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [accessKeyId, setAccessKeyId] = useState("");
  const [secretAccessKey, setSecretAccessKey] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [bucketName, setBucketName] = useState("");
  // Quota sliders (defaults align with free tier thresholds)
  const [storageLimitGB, setStorageLimitGB] = useState<number>(100); // GB
  const [classALimitM, setClassALimitM] = useState<number>(1); // million ops
  const [classBLimitM, setClassBLimitM] = useState<number>(10); // million ops

  const handleCompleteSetup = async () => {
    if (password !== confirmPassword) {
      toast.error("Passwords do not match");
      return;
    }

    if (password.length < 8) {
      toast.error("Password must be at least 8 characters long");
      return;
    }

    setIsProcessing(true);
    try {
      const estimatedMonthlyBudget =
        Math.max(0, storageLimitGB - 10) * 0.015 +
        Math.max(0, classALimitM - 1) * 4.5 +
        Math.max(0, classBLimitM - 10) * 0.36;

      await initializeSystem({
        password,
        r2Config: {
          accessKeyId,
          secretAccessKey,
          endpoint,
          bucketName,
        },
        quotaConfig: {
          monthlyCostLimit: Number(estimatedMonthlyBudget.toFixed(4)),
          storageLimitGB,
          classALimitM,
          classBLimitM,
        },
      });

      toast.success("Setup completed successfully!");
      navigate("/login");
    } catch (error) {
      toast.error(
        error instanceof AppError
          ? error.message
          : "Setup failed. Please check your configuration.",
      );
    } finally {
      setIsProcessing(false);
    }
  };

  const isStepValid = (step: number) => {
    switch (step) {
      case 1:
        return password.length >= 8 && password === confirmPassword;
      case 2:
        return accessKeyId && secretAccessKey && endpoint && bucketName;
      case 3:
        return storageLimitGB >= 10 && classALimitM >= 1 && classBLimitM >= 10;
      default:
        return false;
    }
  };

  const completedSteps = Array.from(
    { length: currentStep - 1 },
    (_, i) => i + 1,
  ).filter((step) => isStepValid(step));
  const progressValue = (completedSteps.length / SETUP_STEPS.length) * 100;

  return (
    <>
      <Helmet>
        <title>Setup - R2Vault</title>
        <meta name="description" content="Set up your R2Vault instance" />
      </Helmet>

      <div className="from-primary/5 to-secondary/5 flex min-h-screen items-center justify-center bg-gradient-to-br p-4">
        <div className="w-full max-w-2xl space-y-8">
          {/* Header */}
          <div className="text-center">
            <div className="mb-6 flex justify-center">
              <div className="relative">
                <div className="bg-primary flex h-16 w-16 items-center justify-center rounded-xl shadow-lg">
                  <img
                    src="/logo.jpg"
                    alt="R2Vault"
                    width={40}
                    height={40}
                    className="rounded-lg"
                  />
                </div>
              </div>
            </div>
            <h1 className="text-foreground flex items-center justify-center gap-3 text-3xl font-bold">
              <Rocket className="text-primary h-8 w-8" />
              Setup R2Vault
            </h1>
            <p className="text-muted-foreground mt-2">
              Configure your cost-controlled storage system
            </p>
          </div>

          {/* Progress */}
          <Card>
            <CardHeader>
              <CardTitle className="text-lg">Setup Progress</CardTitle>
              <Progress value={progressValue} className="mt-2" />
            </CardHeader>
            <CardContent>
              <div className="flex justify-between">
                {SETUP_STEPS.map((step) => (
                  <div
                    key={step.id}
                    className="flex flex-1 flex-col items-center text-center"
                  >
                    <div
                      className={`mb-2 flex h-8 w-8 items-center justify-center rounded-full text-sm font-medium ${
                        completedSteps.includes(step.id)
                          ? "bg-green-500 text-white"
                          : currentStep === step.id
                            ? "bg-primary text-white"
                            : "bg-muted text-muted-foreground"
                      }`}
                    >
                      {completedSteps.includes(step.id) ? (
                        <CheckCircle className="h-4 w-4" />
                      ) : (
                        step.id
                      )}
                    </div>
                    <h3 className="text-sm font-medium">{step.title}</h3>
                    <p className="text-muted-foreground text-xs">
                      {step.description}
                    </p>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>

          {/* Step Content */}
          <Card className="border-0 shadow-xl">
            {currentStep === 1 && (
              <>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Key className="h-5 w-5" />
                    Admin Password
                  </CardTitle>
                  <CardDescription>
                    Create a secure password for your R2Vault admin account
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <Alert>
                    <AlertCircle className="h-4 w-4" />
                    <AlertDescription>
                      Use a strong password with at least 8 characters. This
                      will be used to access your R2Vault dashboard.
                    </AlertDescription>
                  </Alert>

                  <div className="space-y-2">
                    <Label htmlFor="password">Admin Password</Label>
                    <Input
                      id="password"
                      type="password"
                      placeholder="Enter a secure password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                    />
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="confirmPassword">Confirm Password</Label>
                    <Input
                      id="confirmPassword"
                      type="password"
                      placeholder="Confirm your password"
                      value={confirmPassword}
                      onChange={(e) => setConfirmPassword(e.target.value)}
                    />
                  </div>

                  <Button
                    onClick={() => setCurrentStep(2)}
                    disabled={!isStepValid(1)}
                    className="w-full"
                  >
                    Continue to R2 Configuration
                    <ArrowRight className="ml-2 h-4 w-4" />
                  </Button>
                </CardContent>
              </>
            )}

            {currentStep === 2 && (
              <>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Server className="h-5 w-5" />
                    R2 Configuration
                  </CardTitle>
                  <CardDescription>
                    Connect to your Cloudflare R2 storage bucket
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <Alert>
                    <AlertCircle className="h-4 w-4" />
                    <AlertDescription>
                      You'll need your Cloudflare R2 API credentials. Get these
                      from your Cloudflare dashboard under R2 Object Storage.
                    </AlertDescription>
                  </Alert>

                  <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                    <div className="space-y-2">
                      <Label htmlFor="accessKeyId">Access Key ID</Label>
                      <Input
                        id="accessKeyId"
                        placeholder="Your R2 Access Key ID"
                        value={accessKeyId}
                        onChange={(e) => setAccessKeyId(e.target.value)}
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="secretAccessKey">Secret Access Key</Label>
                      <Input
                        id="secretAccessKey"
                        type="password"
                        placeholder="Your R2 Secret Access Key"
                        value={secretAccessKey}
                        onChange={(e) => setSecretAccessKey(e.target.value)}
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="endpoint">Endpoint URL</Label>
                      <Input
                        id="endpoint"
                        placeholder="https://your-account-id.r2.cloudflarestorage.com"
                        value={endpoint}
                        onChange={(e) => setEndpoint(e.target.value)}
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="bucketName">Bucket Name</Label>
                      <Input
                        id="bucketName"
                        placeholder="your-bucket-name"
                        value={bucketName}
                        onChange={(e) => setBucketName(e.target.value)}
                      />
                    </div>
                  </div>

                  <div className="flex gap-3">
                    <Button
                      variant="outline"
                      onClick={() => setCurrentStep(1)}
                      className="flex-1"
                    >
                      Back
                    </Button>
                    <Button
                      onClick={() => setCurrentStep(3)}
                      disabled={!isStepValid(2)}
                      className="flex-1"
                    >
                      Continue to Budget Settings
                      <ArrowRight className="ml-2 h-4 w-4" />
                    </Button>
                  </div>
                </CardContent>
              </>
            )}

            {currentStep === 3 && (
              <>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <DollarSign className="h-5 w-5" />
                    Quota & Budget Settings
                  </CardTitle>
                  <CardDescription>
                    Set per-category limits. Monthly budget is estimated with
                    Cloudflare standard pricing and free tiers.
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <Alert>
                    <AlertCircle className="h-4 w-4" />
                    <AlertDescription>
                      R2Vault will monitor your usage and help prevent
                      unexpected charges by enforcing these limits.
                    </AlertDescription>
                  </Alert>

                  <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                    <div className="space-y-3">
                      <Label>Storage Limit (GB)</Label>
                      <div className="text-muted-foreground flex items-center justify-between text-sm">
                        <span>10 GB</span>
                        <span>{storageLimitGB} GB</span>
                      </div>
                      <input
                        type="range"
                        min={10}
                        max={100}
                        step={1}
                        value={storageLimitGB}
                        onChange={(e) =>
                          setStorageLimitGB(Number(e.target.value))
                        }
                        className="w-full"
                      />
                      <p className="text-muted-foreground text-xs">
                        First 10GB free, then $0.015 per GB-month
                      </p>
                    </div>

                    <div className="space-y-3">
                      <Label>Class A Operations (million)</Label>
                      <div className="text-muted-foreground flex items-center justify-between text-sm">
                        <span>1 M</span>
                        <span>{classALimitM} M</span>
                      </div>
                      <input
                        type="range"
                        min={1}
                        max={20}
                        step={1}
                        value={classALimitM}
                        onChange={(e) =>
                          setClassALimitM(Number(e.target.value))
                        }
                        className="w-full"
                      />
                      <p className="text-muted-foreground text-xs">
                        First 1M free, then $4.50 per million requests
                      </p>
                    </div>

                    <div className="space-y-3">
                      <Label>Class B Operations (million)</Label>
                      <div className="text-muted-foreground flex items-center justify-between text-sm">
                        <span>10 M</span>
                        <span>{classBLimitM} M</span>
                      </div>
                      <input
                        type="range"
                        min={10}
                        max={50}
                        step={1}
                        value={classBLimitM}
                        onChange={(e) =>
                          setClassBLimitM(Number(e.target.value))
                        }
                        className="w-full"
                      />
                      <p className="text-muted-foreground text-xs">
                        First 10M free, then $0.36 per million requests
                      </p>
                    </div>
                  </div>

                  <div className="rounded-md border p-4">
                    <div className="flex flex-col gap-1">
                      <div className="flex items-center justify-between">
                        <span className="text-muted-foreground text-sm">
                          Storage
                        </span>
                        <span className="text-sm">
                          $
                          {storageLimitGB > 10
                            ? (
                                Math.max(0, storageLimitGB - 10) * 0.015
                              ).toFixed(2)
                            : "Free"}
                        </span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-muted-foreground text-sm">
                          Class A
                        </span>
                        <span className="text-sm">
                          $
                          {classALimitM > 1
                            ? (Math.max(0, classALimitM - 1) * 4.5).toFixed(2)
                            : "Free"}
                        </span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-muted-foreground text-sm">
                          Class B
                        </span>
                        <span className="text-sm">
                          $
                          {classBLimitM > 10
                            ? (Math.max(0, classBLimitM - 10) * 0.36).toFixed(2)
                            : "Free"}
                        </span>
                      </div>
                      <div className="mt-2 flex items-center justify-between font-semibold">
                        <span>Estimated Monthly Budget</span>
                        <span>
                          $
                          {(
                            Math.max(0, storageLimitGB - 10) * 0.015 +
                            Math.max(0, classALimitM - 1) * 4.5 +
                            Math.max(0, classBLimitM - 10) * 0.36
                          ).toFixed(2)}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="flex gap-3">
                    <Button
                      variant="outline"
                      onClick={() => setCurrentStep(2)}
                      className="flex-1"
                    >
                      Back
                    </Button>
                    <Button
                      onClick={handleCompleteSetup}
                      disabled={!isStepValid(3) || isProcessing}
                      className="flex-1"
                    >
                      {isProcessing ? (
                        <div className="flex items-center gap-2">
                          <div className="border-primary-foreground/30 border-t-primary-foreground h-4 w-4 animate-spin rounded-full border-2" />
                          Setting up...
                        </div>
                      ) : (
                        <div className="flex items-center gap-2">
                          <Rocket className="h-4 w-4" />
                          Complete Setup
                        </div>
                      )}
                    </Button>
                  </div>
                </CardContent>
              </>
            )}
          </Card>

          {/* Footer */}
          <div className="text-muted-foreground text-center text-sm">
            <p>R2Vault v1.0 - Cost-controlled file storage for everyone</p>
          </div>
        </div>
      </div>
    </>
  );
}
