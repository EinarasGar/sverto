import { cn } from "@/lib/utils"
import { focusRing } from "@/components/primitives/focus-ring"
import { SvertoMark } from "@/components/layout/sverto-mark"

import { COMPANY } from "./company"
import {
  isExternalTarget,
  LANDING_FOOTER_GROUPS,
  type LandingLink,
} from "./links"
import { MetaLabel } from "./section"

function FooterItem({ label, target }: LandingLink) {
  if (target === null) {
    return <li className="text-[12.5px] leading-[1.4] text-ink-3">{label}</li>
  }
  return (
    <li>
      <a
        href={target}
        {...(isExternalTarget(target)
          ? { target: "_blank", rel: "noopener" }
          : {})}
        className={cn(
          "text-[12.5px] leading-[1.4] text-ink-2 transition-colors duration-instant ease-out-quick hover:text-ink",
          focusRing.sm
        )}
      >
        {label}
      </a>
    </li>
  )
}

export function LandingFooter() {
  return (
    <footer className="mt-14 border-t border-border px-5 pt-8 pb-8 md:px-8 lg:mt-20 lg:px-10 lg:pt-11 lg:pb-10 xl:mt-[100px] xl:px-16">
      <div className="mx-auto w-full max-w-[1180px]">
        <div className="grid gap-8 sm:grid-cols-2 lg:grid-cols-[1.4fr_1fr_1fr_1fr] lg:gap-10">
          <div>
            <div className="flex items-center gap-2.5">
              <SvertoMark className="size-5" />
              <span className="text-[15px] leading-none font-bold tracking-[-0.02em]">
                Sverto
              </span>
            </div>
            <p className="mt-3.5 max-w-[280px] text-[12.5px] leading-[1.6] text-pretty text-ink-3">
              Spending, investing and net worth in one double-entry ledger.
              Built in the EU.
            </p>
          </div>
          {LANDING_FOOTER_GROUPS.map((group) => (
            <nav key={group.title} aria-label={group.title}>
              <MetaLabel>{group.title}</MetaLabel>
              <ul className="mt-3.5 flex flex-col gap-2.5">
                {group.links.map((link) => (
                  <FooterItem key={link.label} {...link} />
                ))}
              </ul>
            </nav>
          ))}
        </div>
        <div className="mt-8 flex flex-col gap-4 border-t border-border pt-5 text-[11px] leading-[1.6] text-ink-3 lg:mt-10 lg:flex-row lg:items-start lg:gap-8">
          <div className="flex max-w-[820px] flex-col gap-2 text-pretty">
            <p>
              Sverto is not a bank, a broker or an investment adviser, and
              nothing here is financial advice. Broker data comes from official
              APIs and your own imports; bank connections through Open Banking
              are on the way. Market prices are end-of-day.
            </p>
            <p>
              {COMPANY.name}, a {COMPANY.legalForm} · Company code{" "}
              {COMPANY.code} · {COMPANY.address} · Data about the company is
              held in the {COMPANY.register} ·{" "}
              <a
                href={`mailto:${COMPANY.contactEmail}`}
                className={cn(
                  "text-ink-2 transition-colors duration-instant ease-out-quick hover:text-ink",
                  focusRing.sm
                )}
              >
                {COMPANY.contactEmail}
              </a>
            </p>
          </div>
          <span className="flex-1" />
          <p className="font-mono whitespace-nowrap">© 2026 {COMPANY.name}</p>
        </div>
      </div>
    </footer>
  )
}
