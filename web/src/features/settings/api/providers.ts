export const PROVIDER_KINDS = [
  "truelayer",
  "trading212",
  "enablebanking",
  "ibkr",
] as const

export type ProviderKind = (typeof PROVIDER_KINDS)[number]

export type ProviderCredential = "oauth" | "apiKey"

export interface ProviderKeyFields {
  readonly keyIdLabel: string
  readonly secretLabel: string
  readonly help: string
}

export interface ProviderCatalogueEntry {
  readonly kind: ProviderKind
  readonly mark: string
  readonly name: string
  readonly tagline: string
  readonly description: string
  readonly connectLabel: string
  readonly credential: ProviderCredential
  readonly keyFields?: ProviderKeyFields
}

/**
 * No endpoint lists available providers with display metadata — the kinds are
 * seeded strings on the server — so the catalogue is owned here.
 */
export const PROVIDER_CATALOGUE: readonly ProviderCatalogueEntry[] = [
  {
    kind: "truelayer",
    mark: "TL",
    name: "TrueLayer",
    tagline: "Bank accounts via Open Banking",
    description:
      "Connect your banks through Open Banking. Sverto imports balances and transaction history and keeps them up to date. Read-only — no payment access is ever requested.",
    connectLabel: "Connect a bank",
    credential: "oauth",
  },
  {
    kind: "enablebanking",
    mark: "EB",
    name: "Enable Banking",
    tagline: "European banks via Open Banking",
    description:
      "Connect banks across Europe through one Open Banking gateway. Sverto imports balances and transaction history and keeps them up to date. Read-only — no payment access is ever requested.",
    connectLabel: "Connect a bank",
    credential: "oauth",
  },
  {
    kind: "trading212",
    mark: "T2",
    name: "Trading 212",
    tagline: "Investment account history",
    description:
      "Import orders, dividends and cash movements from your investment account.",
    connectLabel: "Add an API key",
    credential: "apiKey",
  },
  {
    kind: "ibkr",
    mark: "IB",
    name: "Interactive Brokers",
    tagline: "Investment account history",
    description:
      "Import trades, dividends and cash movements from your Interactive Brokers account through a read-only Flex Query.",
    connectLabel: "Add a Flex token",
    credential: "apiKey",
    keyFields: {
      keyIdLabel: "Flex Query ID",
      secretLabel: "Flex token",
      help: "In the IBKR Client Portal, create an Activity Flex Query (Performance & Reports → Flex Queries) with the Account Information, Trades, Cash Transactions, Open Positions and Cash Report sections, XML format and the default date format. Then enable the Flex Web Service there to generate a token.",
    },
  },
]

export function isProviderKind(value: string): value is ProviderKind {
  return (PROVIDER_KINDS as readonly string[]).includes(value)
}

export function providerEntry(
  kind: string
): ProviderCatalogueEntry | undefined {
  return PROVIDER_CATALOGUE.find((entry) => entry.kind === kind)
}

export function providerName(kind: string): string {
  return providerEntry(kind)?.name ?? kind
}

export function providerMark(kind: string): string {
  return providerEntry(kind)?.mark ?? kind.slice(0, 2).toUpperCase()
}
