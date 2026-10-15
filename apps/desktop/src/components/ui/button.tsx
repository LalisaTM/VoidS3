import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/utils";
const variants = cva("button", {
  variants: {
    variant: {
      default: "primary",
      secondary: "secondary",
      ghost: "ghost",
      destructive: "danger",
    },
    size: { default: "", icon: "icon-button", small: "small" },
  },
  defaultVariants: { variant: "secondary", size: "default" },
});
interface Props
  extends
    ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof variants> {
  asChild?: boolean;
}
export function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: Props) {
  const Comp = asChild ? Slot : "button";
  return (
    <Comp
      type="button"
      className={cn(variants({ variant, size }), className)}
      {...props}
    />
  );
}
