import { COMPANY } from "./company"

export const LANDING_SOURCE_URL = "https://github.com/EinarasGar/sverto"
export const LANDING_LICENCE_URL =
  "https://github.com/EinarasGar/sverto/blob/main/LICENSE"
export const LANDING_ANALYTICS_URL = "https://plausible.io/sverto.com"
export const LANDING_APP_STORE_URL =
  "https://apps.apple.com/app/sverto/id6819424663"
export const LANDING_GOOGLE_PLAY_URL =
  "https://play.google.com/store/apps/details?id=com.sverto.app"

export interface LandingLink {
  readonly label: string
  readonly target: string | null
}

export interface LandingNavItem {
  readonly label: string
  readonly target: string
}

export const LANDING_NAV: readonly LandingNavItem[] = [
  { label: "How it works", target: "/#how-it-works" },
  { label: "Investing", target: "/#investing" },
  { label: "Myra", target: "/#myra" },
  { label: "Self-hosting", target: "/#self-hosting" },
  { label: "Pricing", target: "/#pricing" },
  { label: "Docs", target: LANDING_SOURCE_URL },
]

export interface LandingFooterGroup {
  readonly title: string
  readonly links: readonly LandingLink[]
}

export const LANDING_FOOTER_GROUPS: readonly LandingFooterGroup[] = [
  {
    title: "Product",
    links: [
      { label: "How it works", target: "/#how-it-works" },
      { label: "Investing", target: "/#investing" },
      { label: "Myra", target: "/#myra" },
      { label: "Pricing", target: "/#pricing" },
      { label: "Demo ledger", target: null },
    ],
  },
  {
    title: "Open source",
    links: [
      { label: "GitHub", target: LANDING_SOURCE_URL },
      { label: "Self-hosting guide", target: null },
      { label: "AGPL-3.0 licence", target: LANDING_LICENCE_URL },
      { label: "Roadmap", target: null },
      { label: "Changelog", target: null },
    ],
  },
  {
    title: "More",
    links: [
      { label: "Docs", target: LANDING_SOURCE_URL },
      { label: "API reference", target: null },
      { label: "Status", target: null },
      { label: "Contact", target: `mailto:${COMPANY.contactEmail}` },
      { label: "Privacy", target: "/privacy" },
      { label: "Terms", target: "/terms" },
      { label: "Public analytics", target: LANDING_ANALYTICS_URL },
    ],
  },
]

export function isExternalTarget(target: string): boolean {
  return target.startsWith("http")
}
