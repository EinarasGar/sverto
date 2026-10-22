import { ArrowUpRight } from "lucide-react"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/primitives"

import {
  AI_FEATURES_SHARED_DATA,
  PRIVACY_POLICY_LABEL,
  PRIVACY_POLICY_URL,
} from "./copy"

export function AiFeaturesSharedData({ className }: { className?: string }) {
  return (
    <ul
      data-slot="ai-features-shared-data"
      className={cn("flex flex-col gap-[9px]", className)}
    >
      {AI_FEATURES_SHARED_DATA.map((item) => (
        <li
          key={item}
          className="flex gap-[10px] text-[12px] leading-[1.6] text-pretty text-ink-2"
        >
          <span
            aria-hidden
            className="mt-2 size-1 flex-none rounded-full bg-ink-3"
          />
          <span className="min-w-0">{item}</span>
        </li>
      ))}
    </ul>
  )
}

export function PrivacyPolicyLink() {
  return (
    <a
      href={PRIVACY_POLICY_URL}
      target="_blank"
      rel="noreferrer noopener"
      className={cn(
        "font-medium whitespace-nowrap text-brand underline decoration-border-strong underline-offset-2 transition-colors duration-instant ease-out-quick hover:decoration-brand",
        focusRing.chip
      )}
    >
      {PRIVACY_POLICY_LABEL}
      <ArrowUpRight
        aria-hidden
        strokeWidth={2}
        className="ml-px inline-block size-[11px] translate-y-[-1px] text-ink-3"
      />
    </a>
  )
}
