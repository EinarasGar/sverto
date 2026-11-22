import type { AccountSummary, AccountsView } from "@/features/accounts/api"
import type { HoldingsView } from "@/features/portfolio/api"

import type { Property } from "./api"

export interface PropertyLoan {
  accountId: string
  account: AccountSummary | null
  balance: number
}

export interface PropertyFigures {
  value: number | null
  loanBalance: number
  equity: number | null
  loanToValue: number | null
  loans: PropertyLoan[]
}

export function propertyFigures(
  property: Property,
  accounts: AccountsView,
  wholeHoldings: HoldingsView
): PropertyFigures {
  const own = wholeHoldings.byAccountId[property.accountId]
  const value = own === undefined || own.ratelessCount > 0 ? null : own.value
  const loans = property.loanAccountIds.map((accountId): PropertyLoan => ({
    accountId,
    account: accounts.byId[accountId] ?? null,
    balance: wholeHoldings.byAccountId[accountId]?.value ?? 0,
  }))
  const loanBalance = loans.reduce((sum, loan) => sum + loan.balance, 0)

  return {
    value,
    loanBalance,
    equity: value === null ? null : value + loanBalance,
    loanToValue:
      value === null || value <= 0 ? null : Math.abs(loanBalance) / value,
    loans,
  }
}
