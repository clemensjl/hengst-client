import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

/* DESIGN.md 10.1: five variants, three sizes. The default is `secondary`, so a button without a
   variant can never become a second accent button (DESIGN.md 2.5). Pressed fills apply at once
   (active:duration-0); the return transitions over --dur-press. */
const svg = "[&_svg:not([class*='size-'])]";
const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 rounded-md border whitespace-nowrap select-none transition-[background-color,color,border-color] duration-(--dur-press) ease-out active:duration-0 disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        primary:
          "border-transparent bg-primary text-primary-foreground hover:bg-accent-hover active:bg-accent-pressed disabled:border-border disabled:bg-hover disabled:text-text-disabled",
        secondary:
          "border-input bg-secondary text-foreground hover:bg-hover active:bg-selected disabled:border-border disabled:bg-card disabled:text-text-disabled",
        ghost:
          "border-transparent text-foreground hover:bg-hover active:bg-selected disabled:text-text-disabled",
        destructive:
          "border-transparent bg-destructive text-destructive-foreground hover:bg-danger-hover active:bg-danger-pressed disabled:border-border disabled:bg-hover disabled:text-text-disabled",
        link: "border-transparent text-foreground underline decoration-1 underline-offset-[3px] hover:decoration-2 active:text-muted-foreground disabled:text-text-disabled disabled:no-underline",
      },
      size: {
        default: `h-10 text-label ${svg}:size-4`,
        icon: `size-10 text-label ${svg}:size-[18px]`,
        lg: `h-14 min-w-60 text-play ${svg}:size-[18px]`,
      },
    },
    // Padding is set here, not in the variants and sizes, so no two classes ever fight: every
    // button but the icon button and the link has 16 px at the sides.
    compoundVariants: [
      { size: ["default", "lg"], variant: ["primary", "secondary", "ghost", "destructive"], class: "px-4" },
      { size: ["default", "lg"], variant: "link", class: "px-0" },
      { size: "icon", class: "px-0" },
    ],
    defaultVariants: {
      variant: "secondary",
      size: "default",
    },
  },
);

function Button({
  className,
  variant = "secondary",
  size = "default",
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      data-variant={variant}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
