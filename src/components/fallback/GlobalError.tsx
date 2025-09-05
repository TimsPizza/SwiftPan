import { Button } from "@/components/ui/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ExclamationTriangleIcon } from "@radix-ui/react-icons";

interface GlobalErrorProps {
  title?: string;
  description?: string;
  primaryLabel?: string;
  onPrimary?: () => void;
  secondaryLabel?: string;
  onSecondary?: () => void;
}

export const GlobalError = ({
  title = "Something went wrong",
  description,
  primaryLabel,
  onPrimary,
  secondaryLabel,
  onSecondary,
}: GlobalErrorProps) => {
  return (
    <div className="flex min-h-[60vh] w-full items-center justify-center p-4">
      <Card className="mx-auto w-full max-w-lg">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 w-fit rounded-full bg-red-100 p-3 dark:bg-red-900/20">
            <ExclamationTriangleIcon
              width="32"
              height="32"
              className="text-danger"
            />
          </div>
          <CardTitle className="text-lg text-balance md:text-xl">
            {title}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4 text-center">
          {description && (
            <p className="text-muted-foreground text-sm md:text-base">
              {description}
            </p>
          )}
          <div className="flex flex-col gap-2 sm:flex-row sm:justify-center">
            {primaryLabel && onPrimary && (
              <Button onClick={onPrimary} className="w-full sm:w-auto">
                {primaryLabel}
              </Button>
            )}
            {secondaryLabel && onSecondary && (
              <Button
                variant="outline"
                onClick={onSecondary}
                className="w-full sm:w-auto"
              >
                {secondaryLabel}
              </Button>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
};

export default GlobalError;
