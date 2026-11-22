import type { AccountSummary, AccountsView } from "@/features/accounts/api"

import type { PropertiesView } from "./api"

const MORTGAGE_ACCOUNT_TYPE_ID = 7
const LOAN_ACCOUNT_TYPE_ID = 8

export function linkableLoans(
  accounts: AccountsView,
  properties: PropertiesView
): AccountSummary[] {
  return accounts.accounts.filter(
    (account) =>
      (account.accountTypeId === MORTGAGE_ACCOUNT_TYPE_ID ||
        account.accountTypeId === LOAN_ACCOUNT_TYPE_ID) &&
      !properties.byLoanAccountId.has(account.accountId) &&
      !properties.byAccountId.has(account.accountId)
  )
}
