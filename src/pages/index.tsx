import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { BarChart3, FolderOpen, Settings } from "lucide-react";
import { Helmet } from "react-helmet-async";
import { Link } from "react-router-dom";

const HomePage = () => {
  return (
    <>
      <Helmet>
        <title>R2Vault</title>
        <meta name="description" content="R2Vault - Cost-controlled storage" />
      </Helmet>

      <div className="container mx-auto space-y-6 p-6">
        <div className="space-y-2">
          <h1 className="text-3xl font-bold tracking-tight">
            Welcome to R2Vault
          </h1>
          <p className="text-muted-foreground">
            Your cost-controlled Cloudflare R2 storage dashboard.
          </p>
        </div>

        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          <Card className="h-full">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FolderOpen className="h-5 w-5" /> Files
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <p className="text-muted-foreground text-sm">
                Manage your uploaded files.
              </p>
              <Link to="/files">
                <Button className="w-full">Go to Files</Button>
              </Link>
            </CardContent>
          </Card>

          <Card className="h-full">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <BarChart3 className="h-5 w-5" /> Usage & Costs
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <p className="text-muted-foreground text-sm">
                Monitor storage and operation usage.
              </p>
              <Link to="/usage">
                <Button variant="outline" className="w-full">
                  View Analytics
                </Button>
              </Link>
            </CardContent>
          </Card>

          <Card className="h-full">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Settings className="h-5 w-5" /> Settings
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <p className="text-muted-foreground text-sm">
                Configure R2 credentials and quotas.
              </p>
              <Link to="/settings">
                <Button variant="ghost" className="w-full">
                  Open Settings
                </Button>
              </Link>
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
};

export default HomePage;
