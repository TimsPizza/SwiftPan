import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90",
        destructive:
          "bg-destructive text-white shadow-xs hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/60",
        outline:
          "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:border-input dark:hover:bg-input/50",
        secondary:
          "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80",
        ghost:
          "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
        link: "text-primary underline-offset-4 hover:underline",
        // R2Vault custom variants
        danger:
          "bg-danger text-white shadow-xs hover:bg-danger-dark focus-visible:ring-danger/20",
        warning:
          "bg-warning text-white shadow-xs hover:bg-warning-dark focus-visible:ring-warning/20",
        success:
          "bg-success text-white shadow-xs hover:bg-success-dark focus-visible:ring-success/20",
        info: "bg-info text-gray-800 shadow-xs hover:bg-yellow-100 focus-visible:ring-info/20",
      },
      size: {
        default: "h-9 px-4 py-2 has-[>svg]:px-3",
        sm: "h-8 rounded-md gap-1.5 px-3 has-[>svg]:px-2.5",
        xs: "h-7 rounded-md gap-1.5 px-2 has-[>svg]:px-1.5",
        lg: "h-10 rounded-md px-6 has-[>svg]:px-4",
        icon: "size-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

const Button = React.forwardRef<
  HTMLButtonElement,
  React.ComponentProps<"button"> &
    VariantProps<typeof buttonVariants> & {
      asChild?: boolean;
    }
>(({ className, variant, size, asChild = false, ...props }, ref) => {
  const Comp = asChild ? Slot : "button";
  // Fast-press: call onClick on pointerup with small move threshold
  // Only apply when not rendering asChild (since asChild may be a link/NavLink)
  const [ignoreNextClick, setIgnoreNextClick] = React.useState(false);
  const pressData = React.useRef({
    active: false,
    id: -1,
    sx: 0,
    sy: 0,
    moved: false,
  });

  const userOnClick = (props as any).onClick as
    | ((e: React.MouseEvent<HTMLButtonElement>) => void)
    | undefined;
  const userOnPointerDown = (props as any).onPointerDown as
    | ((e: React.PointerEvent<HTMLButtonElement>) => void)
    | undefined;
  const userOnPointerMove = (props as any).onPointerMove as
    | ((e: React.PointerEvent<HTMLButtonElement>) => void)
    | undefined;
  const userOnPointerUp = (props as any).onPointerUp as
    | ((e: React.PointerEvent<HTMLButtonElement>) => void)
    | undefined;

  const THRESH = 10; // px movement threshold to treat as tap

  const handlePointerDown = (e: React.PointerEvent<HTMLButtonElement>) => {
    userOnPointerDown?.(e);
    if (asChild) return; // don't interfere with Slot children (links)
    if (!e.isPrimary || (props as any).disabled) return;
    pressData.current.active = true;
    pressData.current.id = e.pointerId;
    pressData.current.sx = e.clientX;
    pressData.current.sy = e.clientY;
    pressData.current.moved = false;
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLButtonElement>) => {
    userOnPointerMove?.(e);
    if (asChild) return;
    if (!pressData.current.active) return;
    const dx = Math.abs(e.clientX - pressData.current.sx);
    const dy = Math.abs(e.clientY - pressData.current.sy);
    if (dx > THRESH || dy > THRESH) pressData.current.moved = true;
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLButtonElement>) => {
    userOnPointerUp?.(e);
    if (asChild) return;
    if (!pressData.current.active) return;
    const wasTap = !pressData.current.moved && e.isPrimary;
    pressData.current.active = false;
    if (wasTap && typeof userOnClick === "function") {
      // Fire early and swallow the upcoming click to avoid perceived delay
      setIgnoreNextClick(true);
      // Coerce event type to mouse handler signature
      (userOnClick as any)(e);
    }
  };

  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    if (ignoreNextClick) {
      e.preventDefault();
      e.stopPropagation();
      setIgnoreNextClick(false);
      return;
    }
    userOnClick?.(e);
  };

  const rest = props as Omit<React.ComponentProps<"button">, "ref">;

  return (
    <Comp
      ref={ref}
      data-slot="button"
      // default to button to avoid accidental form submits
      {...(!asChild ? { type: (rest.type as any) ?? "button" } : null)}
      className={cn(
        buttonVariants({ variant, size, className }),
        // ensure no 300ms delay even in older engines
        "touch-manipulation active:scale-95 active:opacity-90",
      )}
      // only attach fast-press handlers when not asChild
      {...(!asChild
        ? {
            onPointerDown: handlePointerDown,
            onPointerMove: handlePointerMove,
            onPointerUp: handlePointerUp,
            onClick: handleClick,
          }
        : { onClick: userOnClick })}
      // spread the rest (excluding the handlers we already consumed)
      {...Object.fromEntries(
        Object.entries(rest).filter(
          ([k]) =>
            !["onClick", "onPointerDown", "onPointerMove", "onPointerUp", "type"].includes(
              k,
            ),
        ),
      )}
    />
  );
});

Button.displayName = "Button";

export { Button, buttonVariants };
