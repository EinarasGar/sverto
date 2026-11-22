import { useMemo } from "react"

import type { HoldingsView } from "@/features/portfolio/api"
import { useHoldingsSuspense } from "@/features/portfolio/api"
import type { PropertiesView, Property } from "@/features/properties/api"
import { usePropertiesSuspense } from "@/features/properties/api"
import type { AccountClass } from "@/lib/domain/accounts"
import {
  ACCOUNT_CLASS_LABELS,
  ACCOUNT_CLASS_ORDER,
  isLiabilityClass,
} from "@/lib/domain/accounts"

import type { AccountSummary, AccountsView } from "./accounts"
import { useAccountsSuspense } from "./accounts"

export interface AccountBalance extends AccountSummary {
  /** Sum of this account's holdings. Already the user's share — holdings apply it. */
  value: number
  /** Holdings on this account with no rate path, so `value` is short by that much. */
  ratelessCount: number
  hasHoldings: boolean
  /**
   * Set on a property's own account. Its `loanAccountIds` keep only the loans this list
   * holds, and `value` includes them.
   */
  property: Property | null
}

export interface AccountClassBalanceGroup {
  accountClass: AccountClass
  label: string
  accounts: AccountBalance[]
  subtotal: number
  ratelessCount: number
}

export interface AccountBalancesView {
  accounts: AccountBalance[]
  /** Every class in render order, including empty ones. Liabilities last. */
  groups: AccountClassBalanceGroup[]
  /** Sum over the accounts `GET /accounts` returned. */
  total: number
  /** Sum of every holding, including accounts the list endpoint filtered out. */
  netWorth: number
  /**
   * `netWorth − total`. Non-zero means holdings sit on accounts that
   * `GET /accounts` did not return — deactivated ones. Surface it rather than
   * letting the two totals disagree silently.
   */
  unmatchedValue: number
  unmatchedAccountIds: string[]
  assetsTotal: number
  liabilitiesTotal: number
  liquidTotal: number
  ratelessCount: number
  isDegraded: boolean
}

export function buildAccountBalances(
  accountsView: AccountsView,
  holdingsView: HoldingsView,
  propertiesView: PropertiesView
): AccountBalancesView {
  const isListed = (accountId: string) =>
    accountsView.byId[accountId] !== undefined

  const accounts: AccountBalance[] = accountsView.accounts
    .filter((account) => {
      const owner = propertiesView.byLoanAccountId.get(account.accountId)
      return owner === undefined || !isListed(owner.accountId)
    })
    .map((account) => {
      const linked = propertiesView.byAccountId.get(account.accountId)
      const property =
        linked === undefined
          ? null
          : {
              ...linked,
              loanAccountIds: linked.loanAccountIds.filter(isListed),
            }
      const holdings = [account.accountId, ...(property?.loanAccountIds ?? [])]
        .map((accountId) => holdingsView.byAccountId[accountId])
        .filter((entry) => entry !== undefined)
      return {
        ...account,
        value: holdings.reduce((sum, entry) => sum + entry.value, 0),
        ratelessCount: holdings.reduce(
          (sum, entry) => sum + entry.ratelessCount,
          0
        ),
        hasHoldings: holdings.length > 0,
        property,
      }
    })

  const unmatchedAccountIds = holdingsView.byAccount
    .filter((entry) => accountsView.byId[entry.accountId] === undefined)
    .map((entry) => entry.accountId)

  const groups = ACCOUNT_CLASS_ORDER.map(
    (accountClass): AccountClassBalanceGroup => {
      const members = accounts.filter(
        (account) => account.accountClass === accountClass
      )
      return {
        accountClass,
        label: ACCOUNT_CLASS_LABELS[accountClass],
        accounts: members,
        subtotal: members.reduce((total, account) => total + account.value, 0),
        ratelessCount: members.reduce(
          (total, account) => total + account.ratelessCount,
          0
        ),
      }
    }
  )

  const total = accounts.reduce((sum, account) => sum + account.value, 0)

  return {
    accounts,
    groups,
    total,
    netWorth: holdingsView.totalValue,
    unmatchedValue: holdingsView.totalValue - total,
    unmatchedAccountIds,
    assetsTotal: groups
      .filter((group) => !isLiabilityClass(group.accountClass))
      .reduce((sum, group) => sum + group.subtotal, 0),
    liabilitiesTotal: groups
      .filter((group) => isLiabilityClass(group.accountClass))
      .reduce((sum, group) => sum + group.subtotal, 0),
    liquidTotal: accounts
      .filter((account) => account.isLiquid)
      .reduce((sum, account) => sum + account.value, 0),
    ratelessCount: holdingsView.ratelessCount,
    isDegraded: holdingsView.isDegraded,
  }
}

export interface AccountBalancesQueryParams {
  userId: string
  defaultAssetId: number
}

export function useAccountBalancesSuspense({
  userId,
  defaultAssetId,
}: AccountBalancesQueryParams): AccountBalancesView {
  const accountsView = useAccountsSuspense(userId)
  const holdingsView = useHoldingsSuspense({ userId, defaultAssetId })
  const propertiesView = usePropertiesSuspense(userId)
  return useMemo(
    () => buildAccountBalances(accountsView, holdingsView, propertiesView),
    [accountsView, holdingsView, propertiesView]
  )
}
