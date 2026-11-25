import { useQuery, useSuspenseQuery } from "@tanstack/react-query"

import type { GetHoldingsResponse } from "@/api"
import { PortfolioApiFactory } from "@/api"
import { decimal, decimalOrNull } from "@/lib/domain/decimals"
import { api } from "@/lib/api"
import type { AccountRef, AssetRef } from "@/lib/domain/refs"
import type { PropertiesView, Property } from "@/features/properties/api"
import { apiQueryOptions, queryKeys, STALE_TIMES } from "@/lib/query"

import type { PortfolioLookups } from "./refs"
import { toPortfolioLookups } from "./refs"

/**
 * `apply_ownership_share` defaults to true server-side (portfolio_handler.rs:83), so
 * holdings are the user's share unless a caller asks for whole accounts. The generated
 * client does not expose the parameter, so it travels as a raw query param.
 * `GET /accounts/{id}/portfolio/overview` never applies it — see
 * `appliesOwnershipShare` on the overview views.
 */
export const HOLDINGS_APPLY_OWNERSHIP_SHARE = true

export interface Holding {
  accountId: string
  assetId: number
  units: number
  /** `null` when no rate path reaches the base currency. Render the em dash, never 0. */
  value: number | null
  asset: AssetRef | null
  account: AccountRef | null
}

export interface AccountHoldings {
  accountId: string
  account: AccountRef | null
  value: number
  ratelessCount: number
  holdings: Holding[]
}

export interface AssetHoldings {
  assetId: number
  asset: AssetRef | null
  units: number
  value: number
  ratelessCount: number
  accountCount: number
  holdings: Holding[]
}

export interface HoldingsView {
  holdings: Holding[]
  byAccount: AccountHoldings[]
  byAccountId: Record<string, AccountHoldings | undefined>
  byAsset: AssetHoldings[]
  byAssetId: Record<number, AssetHoldings | undefined>
  /** Sum of the values that exist. Excludes rateless rows. */
  totalValue: number
  ratelessCount: number
  /** At least one holding has no value, so every total on this page is incomplete. */
  isDegraded: boolean
  appliesOwnershipShare: boolean
  lookups: PortfolioLookups
}

function byValueDesc(a: { value: number }, b: { value: number }): number {
  return b.value - a.value
}

function groupByAsset(
  holdings: readonly Holding[]
): Record<number, AssetHoldings | undefined> {
  const byAssetId: Record<number, AssetHoldings | undefined> = {}
  for (const holding of holdings) {
    const rateless = holding.value === null
    const asset = (byAssetId[holding.assetId] ??= {
      assetId: holding.assetId,
      asset: holding.asset,
      units: 0,
      value: 0,
      ratelessCount: 0,
      accountCount: 0,
      holdings: [],
    })
    asset.units += holding.units
    asset.value += holding.value ?? 0
    asset.ratelessCount += rateless ? 1 : 0
    asset.holdings.push(holding)
    asset.accountCount = asset.holdings.length
  }
  return byAssetId
}

function sortedByValue<T extends { value: number }>(
  index: Record<string | number, T | undefined>
): T[] {
  return Object.values(index)
    .filter((entry) => entry !== undefined)
    .sort(byValueDesc)
}

export function buildHoldingsView(
  response: GetHoldingsResponse,
  appliesOwnershipShare: boolean = HOLDINGS_APPLY_OWNERSHIP_SHARE
): HoldingsView {
  const lookups = toPortfolioLookups(response.lookup_tables)

  const holdings: Holding[] = response.holdings.map((row) => ({
    accountId: row.account_id,
    assetId: row.asset_id,
    units: decimal(row.units),
    value: decimalOrNull(row.value),
    asset: lookups.assetsById[row.asset_id] ?? null,
    account: lookups.accountsById[row.account_id] ?? null,
  }))

  const byAccountId: Record<string, AccountHoldings | undefined> = {}
  let totalValue = 0
  let ratelessCount = 0

  for (const holding of holdings) {
    const rateless = holding.value === null
    const value = holding.value ?? 0
    if (rateless) ratelessCount += 1
    else totalValue += value

    const account = (byAccountId[holding.accountId] ??= {
      accountId: holding.accountId,
      account: holding.account,
      value: 0,
      ratelessCount: 0,
      holdings: [],
    })
    account.value += value
    account.ratelessCount += rateless ? 1 : 0
    account.holdings.push(holding)
  }

  const byAssetId = groupByAsset(holdings)

  return {
    holdings,
    byAccount: sortedByValue(byAccountId),
    byAccountId,
    byAsset: sortedByValue(byAssetId),
    byAssetId,
    totalValue,
    ratelessCount,
    isDegraded: ratelessCount > 0,
    appliesOwnershipShare,
    lookups,
  }
}

export interface PropertyHoldings {
  property: Property
  asset: AssetRef | null
  assetValue: number
  loanBalance: number
  value: number
  ratelessCount: number
}

/**
 * The Portfolio page's sections. A property absorbs its own account and every linked
 * loan; any other Credit, Mortgage or Loan account is a liability on its own; what is
 * left is summed per asset, so a currency row never carries debt.
 */
export interface FoldedHoldings {
  assets: AssetHoldings[]
  assetsById: Record<number, AssetHoldings | undefined>
  properties: PropertyHoldings[]
  /** Largest debt first. */
  liabilities: AccountHoldings[]
  holdingsTotal: number
  liabilitiesTotal: number
}

/** Without `properties` (still loading, or failed) property accounts stay plain assets. */
export function foldHoldings(
  view: HoldingsView,
  properties?: PropertiesView
): FoldedHoldings {
  const byPropertyId = new Map<string, PropertyHoldings>()
  const liabilities: AccountHoldings[] = []
  const foldedAccountIds = new Set<string>()

  for (const account of view.byAccount) {
    const property =
      properties?.byAccountId.get(account.accountId) ??
      properties?.byLoanAccountId.get(account.accountId)

    if (property === undefined) {
      if (account.account?.isLiability === true) {
        liabilities.push(account)
        foldedAccountIds.add(account.accountId)
      }
      continue
    }

    foldedAccountIds.add(account.accountId)
    let entry = byPropertyId.get(property.propertyId)
    if (entry === undefined) {
      entry = {
        property,
        asset: view.lookups.assetsById[property.assetId] ?? null,
        assetValue: 0,
        loanBalance: 0,
        value: 0,
        ratelessCount: 0,
      }
      byPropertyId.set(property.propertyId, entry)
    }
    if (account.accountId === property.accountId) {
      entry.assetValue += account.value
    } else {
      entry.loanBalance += account.value
    }
    entry.value += account.value
    entry.ratelessCount += account.ratelessCount
  }

  const assetsById = groupByAsset(
    view.holdings.filter((holding) => !foldedAccountIds.has(holding.accountId))
  )
  const assets = sortedByValue(assetsById)
  const propertyHoldings = [...byPropertyId.values()].sort(byValueDesc)
  const sum = (entries: readonly { value: number }[]) =>
    entries.reduce((total, entry) => total + entry.value, 0)

  return {
    assets,
    assetsById,
    properties: propertyHoldings,
    liabilities: liabilities.sort((a, b) => a.value - b.value),
    holdingsTotal: sum(assets) + sum(propertyHoldings),
    liabilitiesTotal: sum(liabilities),
  }
}

export interface HoldingsQueryParams {
  userId: string
  defaultAssetId: number
  applyOwnershipShare?: boolean
  includeZeroBalances?: boolean
}

export function holdingsQueryOptions({
  userId,
  defaultAssetId,
  applyOwnershipShare = HOLDINGS_APPLY_OWNERSHIP_SHARE,
  includeZeroBalances = false,
}: HoldingsQueryParams) {
  return apiQueryOptions({
    queryKey: queryKeys.user(userId).portfolio.holdings({
      defaultAssetId,
      applyOwnershipShare,
      includeZeroBalances,
    }),
    staleTime: STALE_TIMES.short,
    fetch: async ({ signal }): Promise<HoldingsView> => {
      const response = await api(PortfolioApiFactory).getHoldings(
        userId,
        defaultAssetId,
        includeZeroBalances,
        {
          signal,
          params: { apply_ownership_share: applyOwnershipShare },
        }
      )
      return buildHoldingsView(response.data, applyOwnershipShare)
    },
    meta: { errorContext: "Holdings could not be loaded" },
  })
}

export function useHoldings(params: HoldingsQueryParams) {
  return useQuery(holdingsQueryOptions(params))
}

export function useHoldingsSuspense(params: HoldingsQueryParams): HoldingsView {
  return useSuspenseQuery(holdingsQueryOptions(params)).data
}
