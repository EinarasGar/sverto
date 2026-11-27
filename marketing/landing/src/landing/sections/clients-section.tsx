import { StoreBadges } from "../cta"
import {
  LandingSection,
  SectionEyebrow,
  SectionHeading,
  SectionSplit,
} from "../section"

function ThemedShot({
  dark,
  light,
  alt,
  className,
  width,
  height,
}: {
  dark: string
  light: string
  alt: string
  className?: string
  width: number
  height: number
}) {
  return (
    <>
      <img
        src={dark}
        alt={alt}
        width={width}
        height={height}
        loading="lazy"
        decoding="async"
        className={`hidden dark:block ${className ?? ""}`}
      />
      <img
        src={light}
        alt={alt}
        width={width}
        height={height}
        loading="lazy"
        decoding="async"
        className={`dark:hidden ${className ?? ""}`}
      />
    </>
  )
}

export function ClientsSection() {
  return (
    <LandingSection id="apps" className="scroll-mt-[68px]">
      <SectionEyebrow>07 · The clients</SectionEyebrow>
      <SectionSplit>
        <SectionHeading className="mt-0 max-w-[720px]">
          At your desk. In your pocket. One ledger underneath.
        </SectionHeading>
        <div className="max-w-[420px]">
          <p className="text-[13.5px] leading-[1.6] text-pretty text-ink-2 lg:text-[15px] lg:leading-[1.65]">
            The web app for the monthly review, native iOS and Android apps for
            the receipt you&rsquo;re holding right now. Your accounts,
            transactions and holdings stay together wherever you open Sverto.
          </p>
          <StoreBadges className="mt-4 lg:mt-5" />
        </div>
      </SectionSplit>

      <div className="relative mt-8 pb-[28%] sm:pb-[10%] md:pb-[7%] lg:mt-12">
        <div
          aria-hidden
          className="pointer-events-none absolute inset-x-[-10%] top-[-8%] bottom-[-14%] -z-10 rounded-[50%] bg-brand-dim opacity-70 blur-[90px]"
        />

        <div className="pr-[8%]">
          <div className="overflow-hidden rounded-xl border border-border-strong bg-surface shadow-popover sm:rounded-2xl">
            <div className="hidden h-[34px] items-center gap-2 border-b border-border bg-surface-2 px-3.5 md:flex">
              <span className="size-2 rounded-full bg-border-strong" />
              <span className="size-2 rounded-full bg-border-strong" />
              <span className="size-2 rounded-full bg-border-strong" />
              <div className="flex flex-1 justify-center">
                <span className="rounded-sm bg-background px-3 py-[3px] font-mono text-[10.5px] leading-none text-ink-3">
                  app.sverto.com
                </span>
              </div>
            </div>
            <div className="relative aspect-[1600/826] overflow-clip">
              <ThemedShot
                dark="/shots/ledger-dark.png"
                light="/shots/ledger-light.png"
                alt="The Sverto web dashboard showing net worth, a history chart, account balances and recent transactions."
                width={1434}
                height={1190}
                className="absolute inset-x-0 top-0 block h-auto w-full"
              />
            </div>
          </div>
        </div>

        <div className="absolute right-0 bottom-0 z-10 flex w-[58%] items-end gap-[3%] sm:w-[42%] lg:w-[38%]">
          <figure className="relative z-10 w-1/2 shrink-0">
            <figcaption className="absolute top-full mt-2 w-full text-center text-[11px] font-medium tracking-wide text-ink-2 sm:text-xs">
              iPhone
            </figcaption>
            <div className="overflow-hidden rounded-[14px] border-[3px] border-border-strong bg-surface shadow-popover sm:rounded-[20px] sm:border-[5px] lg:rounded-[28px] lg:border-[6px]">
              <ThemedShot
                dark="/shots/ios-dark.png"
                light="/shots/ios-light.png"
                alt="The native Sverto iOS app showing portfolio net worth, a history chart, holdings and the bottom navigation tabs."
                width={640}
                height={1391}
                className="block h-auto w-full"
              />
            </div>
          </figure>

          <figure className="relative w-[47%] shrink-0 -translate-y-[9%]">
            <figcaption className="absolute top-full mt-2 w-full text-center text-[11px] font-medium tracking-wide text-ink-2 sm:text-xs">
              Android
            </figcaption>
            <div className="overflow-hidden rounded-[14px] border-[3px] border-border-strong bg-surface shadow-popover sm:rounded-[20px] sm:border-[5px] lg:rounded-[28px] lg:border-[6px]">
              <ThemedShot
                dark="/shots/phone-dark.png"
                light="/shots/phone-light.png"
                alt="The native Sverto Android app showing net worth, a portfolio chart, holdings and the bottom navigation tabs."
                width={640}
                height={1436}
                className="block h-auto w-full"
              />
            </div>
          </figure>
        </div>
      </div>
    </LandingSection>
  )
}
