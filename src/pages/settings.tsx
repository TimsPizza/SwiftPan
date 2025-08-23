"use client";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
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
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useSettings } from "@/hooks/use-settings";
import {
  AlertCircle,
  CheckCircle,
  DollarSign,
  Key,
  Save,
  Server,
  Settings as SettingsIcon,
  Shield,
} from "lucide-react";
import { Helmet } from "react-helmet-async";
import { useEffect, useState } from "react";
import { toast } from "sonner";

export default function SettingsPage() {
  const {
    config,
    quotaConfig,
    loading,
    error,
    updateR2Credentials,
    updateQuotaBudget,
    updatePassword,
  } = useSettings();
  const [isUpdating, setIsUpdating] = useState(false);

  // R2 core
  const [accessKeyId, setAccessKeyId] = useState("");
  const [secretAccessKey, setSecretAccessKey] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [bucketName, setBucketName] = useState("");
  // System credentials
  const [password, setPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  // Quota
  const [storageLimitGB, setStorageLimitGB] = useState<number>(100);
  const [classALimitM, setClassALimitM] = useState<number>(1); // million ops
  const [classBLimitM, setClassBLimitM] = useState<number>(10); // million ops
  const [enableNotifications, setEnableNotifications] = useState(true);

  const handleSaveR2Config = async () => {
    setIsUpdating(true);
    try {
      await updateR2Credentials({
        accessKeyId,
        secretAccessKey,
        endpoint,
        bucketName,
      });
      toast.success("R2 configuration saved successfully");
    } catch (error) {
      toast.error("Failed to save R2 configuration");
    } finally {
      setIsUpdating(false);
    }
  };

  const estimatedMonthlyBudget =
    Math.max(0, storageLimitGB - 10) * 0.015 +
    Math.max(0, classALimitM - 1) * 4.5 +
    Math.max(0, classBLimitM - 10) * 0.36;

  // Initialize sliders from loaded quotaConfig
  useEffect(() => {
    if (!quotaConfig) return;
    if (typeof quotaConfig.storageLimitGB === "number") {
      setStorageLimitGB(quotaConfig.storageLimitGB);
    }
    if (typeof quotaConfig.classALimitM === "number") {
      setClassALimitM(quotaConfig.classALimitM);
    }
    if (typeof quotaConfig.classBLimitM === "number") {
      setClassBLimitM(quotaConfig.classBLimitM);
    }
  }, [quotaConfig]);

  const handleSaveQuotas = async () => {
    setIsUpdating(true);
    try {
      await updateQuotaBudget({
        monthlyCostLimit: Number(estimatedMonthlyBudget.toFixed(4)),
        storageLimitGB,
        classALimitM,
        classBLimitM,
      });
      toast.success("Quotas saved successfully");
    } catch (error) {
      toast.error("Failed to save quotas");
    } finally {
      setIsUpdating(false);
    }
  };

  const handleChangePassword = async () => {
    if (newPassword !== confirmPassword) {
      toast.error("Passwords do not match");
      return;
    }

    setIsUpdating(true);
    try {
      await updatePassword({
        currentPassword: password,
        newPassword: newPassword,
      });
      toast.success("Password changed successfully");
      setPassword("");
      setNewPassword("");
      setConfirmPassword("");
    } catch (error) {
      toast.error("Failed to change password");
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <>
      <Helmet>
        <title>Settings - R2Vault</title>
        <meta name="description" content="Configure your R2Vault settings" />
      </Helmet>

      <div className="container mx-auto space-y-6 p-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="flex items-center gap-3 text-3xl font-bold tracking-tight">
              <SettingsIcon className="text-primary h-8 w-8" />
              Settings
            </h1>
            <p className="text-muted-foreground mt-1">
              Configure your R2Vault instance
            </p>
          </div>
          <Badge variant="secondary" className="bg-green-100 text-green-800">
            <CheckCircle className="mr-1 h-3 w-3" />
            Configured
          </Badge>
        </div>

        <Tabs defaultValue="r2-config" className="space-y-6">
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="r2-config">R2 Configuration</TabsTrigger>
            <TabsTrigger value="quotas">Quotas & Limits</TabsTrigger>
            <TabsTrigger value="security">Security</TabsTrigger>
          </TabsList>

          <TabsContent value="r2-config" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Server className="h-5 w-5" />
                  Cloudflare R2 Configuration
                </CardTitle>
                <CardDescription>
                  Configure your Cloudflare R2 storage backend
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <Alert>
                  <AlertCircle className="h-4 w-4" />
                  <AlertDescription>
                    Anyone with these credentials can access your R2 storage.
                    NEVER share any of these for any reason!
                  </AlertDescription>
                </Alert>

                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <div className="space-y-2">
                    <Label htmlFor="accessKeyId">Access Key ID</Label>
                    <Input
                      id="accessKeyId"
                      placeholder="Enter your R2 Access Key ID"
                      value={accessKeyId}
                      onChange={(e) => setAccessKeyId(e.target.value)}
                    />
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="secretAccessKey">Secret Access Key</Label>
                    <Input
                      id="secretAccessKey"
                      type="password"
                      placeholder="Enter your R2 Secret Access Key"
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
                      placeholder="your-r2-bucket-name"
                      value={bucketName}
                      onChange={(e) => setBucketName(e.target.value)}
                    />
                  </div>
                </div>

                <Button
                  onClick={handleSaveR2Config}
                  disabled={isUpdating}
                  className="w-full md:w-auto"
                >
                  <Save className="mr-2 h-4 w-4" />
                  {isUpdating ? "Saving..." : "Save R2 Configuration"}
                </Button>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="quotas" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <DollarSign className="h-5 w-5" />
                  Budget & Quota Settings
                </CardTitle>
                <CardDescription>
                  Configure per-category limits. Monthly budget is estimated by
                  standard pricing.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-6">
                <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
                  <div className="space-y-3">
                    <Label>Storage Limit (GB)</Label>
                    <div className="text-muted-foreground flex items-center justify-between text-sm">
                      <span>1 GB</span>
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
                      Billed at $0.015 per GB-month
                    </p>
                  </div>

                  <div className="space-y-3">
                    <Label>Class A Operations (million)</Label>
                    <div className="text-muted-foreground flex items-center justify-between text-sm">
                      <span>0 M</span>
                      <span>{classALimitM} M</span>
                    </div>
                    <input
                      type="range"
                      min={1}
                      max={20}
                      step={1}
                      value={classALimitM}
                      onChange={(e) => setClassALimitM(Number(e.target.value))}
                      className="w-full"
                    />
                    <p className="text-muted-foreground text-xs">
                      Billed at $4.50 per million requests
                    </p>
                  </div>

                  <div className="space-y-3">
                    <Label>Class B Operations (million)</Label>
                    <div className="text-muted-foreground flex items-center justify-between text-sm">
                      <span>0 M</span>
                      <span>{classBLimitM} M</span>
                    </div>
                    <input
                      type="range"
                      min={10}
                      max={50}
                      step={1}
                      value={classBLimitM}
                      onChange={(e) => setClassBLimitM(Number(e.target.value))}
                      className="w-full"
                    />
                    <p className="text-muted-foreground text-xs">
                      Billed at $0.36 per million requests
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
                          ? (Math.max(0, storageLimitGB - 10) * 0.015).toFixed(
                              2,
                            )
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
                      <span>${estimatedMonthlyBudget.toFixed(2)}</span>
                    </div>
                  </div>
                </div>

                <div className="flex items-center space-x-2">
                  <Switch
                    id="notifications"
                    checked={enableNotifications}
                    onCheckedChange={setEnableNotifications}
                  />
                  <Label htmlFor="notifications">
                    Enable budget alerts and notifications
                  </Label>
                </div>

                <Button
                  onClick={handleSaveQuotas}
                  disabled={isUpdating}
                  className="w-full md:w-auto"
                >
                  <Save className="mr-2 h-4 w-4" />
                  {isUpdating ? "Saving..." : "Save Quota Settings"}
                </Button>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="security" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Shield className="h-5 w-5" />
                  Security Settings
                </CardTitle>
                <CardDescription>
                  Manage your account security and access
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <Alert>
                  <Key className="h-4 w-4" />
                  <AlertDescription>
                    Use a strong password with at least 8 characters, including
                    uppercase, lowercase, and numbers.
                  </AlertDescription>
                </Alert>

                <div className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="currentPassword">Current Password</Label>
                    <Input
                      id="currentPassword"
                      type="password"
                      placeholder="Enter current password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                    />
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="newPassword">New Password</Label>
                    <Input
                      id="newPassword"
                      type="password"
                      placeholder="Enter new password"
                      value={newPassword}
                      onChange={(e) => setNewPassword(e.target.value)}
                    />
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="confirmPassword">
                      Confirm New Password
                    </Label>
                    <Input
                      id="confirmPassword"
                      type="password"
                      placeholder="Confirm new password"
                      value={confirmPassword}
                      onChange={(e) => setConfirmPassword(e.target.value)}
                    />
                  </div>
                </div>

                <Button
                  onClick={handleChangePassword}
                  disabled={
                    isUpdating || !password || !newPassword || !confirmPassword
                  }
                  className="w-full md:w-auto"
                >
                  <Key className="mr-2 h-4 w-4" />
                  {isUpdating ? "Changing..." : "Change Password"}
                </Button>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </>
  );
}
