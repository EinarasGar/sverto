import type { PropertyKind } from "@/api"
import type {
  AssetHolding,
  AssetPosition,
  FoldedHoldings,
  PortfolioOverviewView,
} from "@/features/portfolio/api"
import { countOf } from "@/lib/format"
import { accountTypeTaxonomy } from "@/lib/domain/accounts"
import type { AccountRef, AssetRef } from "@/lib/domain/refs"
import {
  accountLabel,
  assetDisplayName,
  isCurrencyAsset,
  isPhysicalAssetType,
  readableAssetLabel,
} from "@/lib/domain/refs"

export interface HoldingAccountRow {
  key: string
  accountId: string
  account: AccountRef | null
  label: string
  units: number | null
  value: number | null
  position: AssetPosition | null
}

export interface PropertyRowPart {
  propertyId: string
  kind: PropertyKind
  assetValue: number
  loanBalance: number
}

export interface PortfolioHoldingRow {
  key: string
  assetId: number
  asset: AssetRef | null
  label: string
  subLabel: string | null
  ticker: string
  isCash: boolean
  units: number | null
  value: number
  /** Against `holdingsShareBasis`, never net worth. Negative for a balance you owe. */
  share: number
  ratelessCount: number
  accounts: HoldingAccountRow[]
  /** `null` for cash and for anything the overview has no cost basis for. */
  lifetime: AssetHolding | null
  /** Set on a property's equity row, which links to the property instead of expanding. */
  property: PropertyRowPart | null
}

export interface LiabilityRow {
  key: string
  accountId: string
  label: string
  typeLabel: string
  value: number
}

function subLabelOf(asset: AssetRef | null, accountCount: number): string {
  const account = countOf(accountCount, "account")
  if (asset === null) return account
  const name = assetDisplayName(asset)
  if (name === readableAssetLabel(asset)) return account
  return `${name} · ${account}`
}

/**
 * Net worth is the wrong denominator for a share: an overdrawn account's negative cash
 * shrinks it below the assets it sits beside, which is how a single holding reaches 104%
 * of the "portfolio". `AllocationBar` already draws its legend against the positive
 * segments, so measuring both against the same basis is what keeps the legend and the
 * table agreeing.
 */
export function holdingsShareBasis(rows: readonly { value: number }[]): number {
  return rows.reduce(
    (total, row) => (row.value > 0 ? total + row.value : total),
    0
  )
}

/**
 * Holdings carry cash and rateless rows; the overview carries cost basis and gains
 * and drops cash entirely. Rows come from holdings so nothing a user owns is missing,
 * and the lifetime half is attached where the overview has one.
 */
export function buildHoldingRows(
  folded: FoldedHoldings,
  overview: PortfolioOverviewView
): PortfolioHoldingRow[] {
  const total = holdingsShareBasis([...folded.assets, ...folded.properties])

  const assetRows = folded.assets.map((entry): PortfolioHoldingRow => {
    const asset = entry.asset
    const physical = isPhysicalAssetType(asset?.assetTypeId)
    const positionsByAccount = new Map(
      (overview.assetsById[entry.assetId]?.positions ?? []).map((position) => [
        position.accountId,
        position,
      ])
    )

    const accounts: HoldingAccountRow[] = [...entry.holdings]
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0))
      .map((holding) => ({
        key: `${String(entry.assetId)}:${holding.accountId}`,
        accountId: holding.accountId,
        account: holding.account,
        label:
          holding.account === null
            ? "Unknown account"
            : accountLabel(holding.account),
        units: physical ? null : holding.units,
        value: holding.value,
        position: positionsByAccount.get(holding.accountId) ?? null,
      }))

    return {
      key: String(entry.assetId),
      assetId: entry.assetId,
      asset,
      label:
        asset === null
          ? `Asset ${String(entry.assetId)}`
          : readableAssetLabel(asset),
      subLabel: subLabelOf(asset, entry.accountCount),
      ticker:
        asset === null ? String(entry.assetId) : readableAssetLabel(asset),
      isCash: asset !== null && isCurrencyAsset(asset),
      units: physical ? null : entry.units,
      value: entry.value,
      share: total === 0 ? 0 : entry.value / total,
      ratelessCount: entry.ratelessCount,
      accounts,
      lifetime: overview.assetsById[entry.assetId] ?? null,
      property: null,
    }
  })

  const propertyRows = folded.properties.map((entry): PortfolioHoldingRow => ({
    key: `property:${entry.property.propertyId}`,
    assetId: entry.property.assetId,
    asset: entry.asset,
    label: entry.property.name,
    subLabel: null,
    ticker: entry.property.name,
    isCash: false,
    units: null,
    value: entry.value,
    share: total === 0 ? 0 : entry.value / total,
    ratelessCount: entry.ratelessCount,
    accounts: [],
    lifetime: null,
    property: {
      propertyId: entry.property.propertyId,
      kind: entry.property.kind,
      assetValue: entry.assetValue,
      loanBalance: entry.loanBalance,
    },
  }))

  return [...assetRows, ...propertyRows].sort((a, b) => b.value - a.value)
}

export function buildLiabilityRows(folded: FoldedHoldings): LiabilityRow[] {
  return folded.liabilities.map((entry) => {
    const typeId = entry.account?.accountTypeId ?? null
    return {
      key: entry.accountId,
      accountId: entry.accountId,
      label:
        entry.account === null
          ? "Unknown account"
          : accountLabel(entry.account),
      typeLabel:
        accountTypeTaxonomy(typeId === null ? null : { id: typeId })?.name ??
        "Liability",
      value: entry.value,
    }
  })
}

export interface HoldingsSummary {
  rows: PortfolioHoldingRow[]
  liabilities: LiabilityRow[]
  holdingsTotal: number
  liabilitiesTotal: number
  /** Denominator of every share in the table and every slice in the composition bar. */
  shareBasis: number
  /** A share column that cannot total 100%, because something here is owed. */
  hasNegativeRow: boolean
  totalGains: number | null
  assetCount: number
  accountCount: number
  ratelessCount: number
  cashValue: number
  pricedValue: number
}

export function summariseHoldings(
  folded: FoldedHoldings,
  overview: PortfolioOverviewView
): HoldingsSummary {
  const rows = buildHoldingRows(folded, overview)
  const liabilities = buildLiabilityRows(folded)
  const accountIds = new Set<string>()
  let cashValue = 0
  let ratelessCount = 0
  for (const row of rows) {
    for (const account of row.accounts) accountIds.add(account.accountId)
    if (row.isCash) cashValue += row.value
    ratelessCount += row.ratelessCount
  }

  return {
    rows,
    liabilities,
    holdingsTotal: folded.holdingsTotal,
    liabilitiesTotal: folded.liabilitiesTotal,
    shareBasis: holdingsShareBasis(rows),
    hasNegativeRow: rows.some((row) => row.value < 0),
    totalGains: rows.some((row) => row.lifetime !== null)
      ? overview.totals.totalGains
      : null,
    assetCount: rows.length,
    accountCount: accountIds.size,
    ratelessCount,
    cashValue,
    pricedValue: overview.totals.marketValue,
  }
}

export interface LotSummary {
  lotCount: number
  heldSince: number | null
  averageUnitCost: number | null
  realisedGains: number
  totalFees: number
}

export function lotSummaryOf(holding: AssetHolding): LotSummary {
  return {
    lotCount: holding.lots.length,
    heldSince: holding.heldSince,
    averageUnitCost: holding.averageUnitCost,
    realisedGains: holding.realisedGains,
    totalFees: holding.totalFees,
  }
}
