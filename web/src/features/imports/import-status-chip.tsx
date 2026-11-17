import { cn } from "@/lib/utils"

import { TONE_CLASSES, type ImportTone } from "./status"

export function ImportStatusChip({
  tone,
  label,
  className,
}: {
  tone: ImportTone
  label: string
  className?: string
}) {
  return (
    <span
      data-slot="import-status"
      data-tone={tone}
      className={cn(
        "inline-flex flex-none items-center rounded-chip border px-[6px] py-[4px] font-sans text-[9.5px] leading-none font-semibold tracking-[0.06em] whitespace-nowrap uppercase",
        TONE_CLASSES[tone],
        className
      )}
    >
      {label}
    </span>
  )
}
