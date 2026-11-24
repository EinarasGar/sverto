import type { ReactNode } from "react"

import { Figure } from "@/components/figure"

import type { NativeAmount } from "../api"
import { nativeFigureProps } from "../api"

export function TransactionTile({
  eyebrow,
  control,
  amount,
  description,
  meta,
}: {
  eyebrow: string
  control?: ReactNode
  amount: NativeAmount | null
  description: string
  meta: string
}) {
  return (
    <div className="flex min-w-0 flex-col gap-[7px] bg-surface px-[13px] py-[11px]">
      <div className="flex min-h-[22px] flex-wrap items-center justify-between gap-2">
        <span className="text-[9.5px] leading-none font-semibold tracking-[0.1em] text-ink-3 uppercase">
          {eyebrow}
        </span>
        {control}
      </div>
      {amount === null ? (
        <Figure value={null} />
      ) : (
        <Figure {...nativeFigureProps(amount)} sign="always" />
      )}
      <p className="truncate text-[12.5px] leading-[1.3] font-medium">
        {description}
      </p>
      <p className="text-[11px] leading-[1.4] text-ink-3">{meta}</p>
    </div>
  )
}
