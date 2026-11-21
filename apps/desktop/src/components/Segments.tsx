import { Radio } from "@base-ui/react/radio";
import { CheckIcon } from "lucide-react";
import { RadioGroup } from "@/components/ui/radio-group";
import { cn } from "@/lib/utils";

export interface SegmentOption<V extends string> {
  value: V;
  label: string;
}

interface Props<V extends string> {
  value: V;
  options: SegmentOption<V>[];
  onChange: (value: V) => void;
  /** The id of the visible label (the section title). */
  labelledBy: string;
  disabled?: boolean;
  className?: string;
}

/**
 * DESIGN.md 10.6: a radio group shown as separate segments (language, theme, loader). One tab stop
 * for the group, arrow keys move and select, a 16 px check slot is always reserved so a segment
 * never changes width.
 */
export function Segments<V extends string>({
  value,
  options,
  onChange,
  labelledBy,
  disabled,
  className,
}: Props<V>) {
  return (
    <RadioGroup
      value={value}
      onValueChange={(v) => onChange(v as V)}
      aria-labelledby={labelledBy}
      disabled={disabled}
      className={className}
    >
      {options.map((o) => (
        <Radio.Root
          key={o.value}
          value={o.value}
          className={cn(
            "flex h-10 items-center gap-2 rounded-md border border-input bg-secondary pr-4 pl-3 text-small text-foreground select-none hover:bg-hover data-checked:border-foreground data-checked:bg-selected data-disabled:text-text-disabled",
          )}
        >
          <span className="flex size-4 shrink-0 items-center justify-center">
            <Radio.Indicator>
              <CheckIcon className="size-4" aria-hidden="true" />
            </Radio.Indicator>
          </span>
          {o.label}
        </Radio.Root>
      ))}
    </RadioGroup>
  );
}
