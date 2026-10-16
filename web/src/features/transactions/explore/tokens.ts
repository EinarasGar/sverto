import { useQuery } from "@tanstack/react-query"
import { useMemo } from "react"

import { useUserId } from "@/auth"
import { useAccounts } from "@/features/accounts/api"
import {
  assetQueryOptions,
  userAssetsQueryOptions,
} from "@/features/assets/api"
import type { TransactionTypeTag } from "@/lib/domain/transaction-types"
import {
  isTransactionTypeTag,
  transactionTypeName,
  TRANSACTION_TYPES,
} from "@/lib/domain/transaction-types"
import type { AccountId, AssetId, CategoryId } from "@/lib/query"

import type { LedgerFilterToken } from "../api"

import { useCategoryNames } from "./category-names"
import type { GroupByMode } from "./pivot"

export interface ExploreSearch {
  readonly account?: string
  readonly asset?: number
  readonly q?: string
  readonly category?: number
  readonly type?: string
  readonly from?: string
  readonly to?: string
  readonly group?: GroupByMode
  readonly tx?: string
  readonly expand?: string
}

export type ExploreSearchPatch = {
  readonly [K in keyof ExploreSearch]?: ExploreSearch[K] | undefined
}

/**
 * `push` for anything that opens a surface, so Back closes it; `replace` for the rest, so
 * Back is not a walk back through every filter the user tried.
 */
export type ExploreSearchHistory = "push" | "replace" | "back"

export type ExplorePatchSearch = (
  patch: ExploreSearchPatch,
  history?: ExploreSearchHistory
) => void

export function readKeys(value: string | undefined): ReadonlySet<string> {
  if (value === undefined) return new Set()
  return new Set(value.split(",").filter((key) => key !== ""))
}

export function writeKeys(keys: ReadonlySet<string>): string | undefined {
  if (keys.size === 0) return undefined
  return [...keys].join(",")
}

export interface TokenLabels {
  readonly accountName: (accountId: AccountId) => string
  readonly categoryName: (categoryId: CategoryId) => string
  readonly assetName: (assetId: AssetId) => string
}

export const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/

export function isIsoDate(value: string): boolean {
  return ISO_DATE.test(value) && !Number.isNaN(Date.parse(`${value}T00:00:00`))
}

export function searchType(search: ExploreSearch): TransactionTypeTag | null {
  const value = search.type
  if (value === undefined) return null
  return isTransactionTypeTag(value) ? value : null
}

/**
 * Order is load-bearing: `planLedgerQuery` demotes a text token to unsupported whenever an
 * account or asset token is present, so text is emitted first to keep the demoted chip in the place
 * the user typed it.
 */
export function buildLedgerTokens(
  search: ExploreSearch,
  labels: TokenLabels
): LedgerFilterToken[] {
  const tokens: LedgerFilterToken[] = []

  const text = search.q?.trim() ?? ""
  if (text !== "") tokens.push({ key: "text", value: text })

  if (search.account !== undefined && search.account !== "") {
    tokens.push({
      key: "account",
      accountId: search.account,
      label: labels.accountName(search.account),
    })
  }

  if (search.asset !== undefined) {
    tokens.push({
      key: "asset",
      assetId: search.asset,
      label: labels.assetName(search.asset),
    })
  }

  if (search.category !== undefined) {
    tokens.push({
      key: "category",
      categoryId: search.category,
      label: labels.categoryName(search.category),
    })
  }

  const type = searchType(search)
  if (type !== null) {
    tokens.push({
      key: "type",
      transactionType: type,
      label: transactionTypeName(type),
    })
  }

  if (search.from !== undefined && isIsoDate(search.from)) {
    tokens.push({ key: "dateFrom", value: search.from })
  }
  if (search.to !== undefined && isIsoDate(search.to)) {
    tokens.push({ key: "dateTo", value: search.to })
  }

  return tokens
}

export function useTokenLabels(assetId?: AssetId): TokenLabels {
  const userId = useUserId()
  const accounts = useAccounts(userId)
  const categoryNames = useCategoryNames(userId)
  const userAssets = useQuery({
    ...userAssetsQueryOptions(userId),
    enabled: assetId !== undefined,
  })
  const customAsset =
    assetId === undefined
      ? undefined
      : userAssets.data?.find((asset) => asset.assetId === assetId)
  const labelQuery = assetQueryOptions(assetId ?? 0)
  const marketAsset = useQuery({
    ...labelQuery,
    meta: { ...labelQuery.meta, suppressGlobalError: true },
    enabled:
      assetId !== undefined &&
      userAssets.isSuccess &&
      customAsset === undefined,
  })
  const byId = accounts.data?.byId
  const assetTicker =
    customAsset?.ticker ?? customAsset?.name ?? marketAsset.data?.ticker ?? null

  return useMemo(
    () => ({
      accountName: (accountId) => byId?.[accountId]?.name ?? accountId,
      categoryName: (categoryId) =>
        categoryNames(categoryId) ?? `Category ${String(categoryId)}`,
      assetName: (id) => assetTicker ?? `Asset ${String(id)}`,
    }),
    [byId, categoryNames, assetTicker]
  )
}

export function useLedgerTokens(search: ExploreSearch): LedgerFilterToken[] {
  const labels = useTokenLabels(search.asset)
  return useMemo(() => buildLedgerTokens(search, labels), [search, labels])
}

export function clearToken(token: LedgerFilterToken): ExploreSearchPatch {
  switch (token.key) {
    case "text":
      return { q: undefined }
    case "account":
      return { account: undefined }
    case "asset":
      return { asset: undefined }
    case "category":
      return { category: undefined }
    case "type":
      return { type: undefined }
    case "dateFrom":
      return { from: undefined }
    case "dateTo":
      return { to: undefined }
    default:
      return {}
  }
}

export const CLEARED_SEARCH: Required<
  Record<keyof Omit<ExploreSearch, "group" | "tx" | "expand">, undefined>
> = {
  account: undefined,
  asset: undefined,
  q: undefined,
  category: undefined,
  type: undefined,
  from: undefined,
  to: undefined,
}

export type TokenInputResult =
  | { readonly ok: true; readonly patch: ExploreSearchPatch }
  | { readonly ok: false; readonly message: string }

function matchType(value: string): TransactionTypeTag | null {
  const wanted = value.trim().toLowerCase()
  if (isTransactionTypeTag(wanted)) return wanted
  return (
    TRANSACTION_TYPES.find(
      (type) => transactionTypeName(type).toLowerCase() === wanted
    ) ?? null
  )
}

/**
 * An unrecognised `key:` prefix parses as free text rather than an error, so a colon inside
 * a real description still searches.
 */
export function parseTokenInput(input: string): TokenInputResult | null {
  const trimmed = input.trim()
  if (trimmed === "") return null

  const separator = trimmed.indexOf(":")
  const key = separator === -1 ? "" : trimmed.slice(0, separator).toLowerCase()
  const value = separator === -1 ? "" : trimmed.slice(separator + 1).trim()

  if (key === "from" || key === "to") {
    if (!isIsoDate(value)) {
      return {
        ok: false,
        message: `“${value}” is not a date. Write it as ${key}:YYYY-MM-DD.`,
      }
    }
    return { ok: true, patch: key === "from" ? { from: value } : { to: value } }
  }

  if (key === "type") {
    const type = matchType(value)
    if (type === null) {
      return {
        ok: false,
        message: `“${value}” is not a transaction type. Try type:${transactionTypeName("regular").toLowerCase()}.`,
      }
    }
    return { ok: true, patch: { type } }
  }

  return { ok: true, patch: { q: trimmed } }
}
