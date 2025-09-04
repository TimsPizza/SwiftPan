import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Helmet } from "react-helmet-async";
import { Link } from "react-router-dom";

const NotFoundPage = () => {
  return (
    <>
      <Helmet>
        <title>Page Not Found - R2Vault</title>
        <meta name="robots" content="noindex" />
      </Helmet>
      <div className="flex min-h-[60vh] items-center justify-center p-6">
        <Card className="w-full max-w-lg">
          <CardHeader>
            <CardTitle>404 - Page Not Found</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-muted-foreground">
              The page you are looking for does not exist or has been moved.
            </p>
            <div className="flex gap-3">
              <Link to="/">
                <Button>Back to Home</Button>
              </Link>
              <Link to="/files">
                <Button variant="outline">Go to Files</Button>
              </Link>
            </div>
          </CardContent>
        </Card>
      </div>
    </>
  );
};

export default NotFoundPage;
