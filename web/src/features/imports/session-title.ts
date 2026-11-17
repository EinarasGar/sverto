import { useUserId } from "@/auth"
import { useAccounts } from "@/features/accounts/api"
import { formatDateStamp } from "@/lib/format"

export function useAccountNames(accountIds: readonly string[]): string[] {
  const accounts = useAccounts(useUserId())
  return accountIds.flatMap((id) => {
    const account = accounts.data?.byId[id]
    return account === undefined ? [] : [account.name]
  })
}

export function importSessionTitle(
  createdAt: string,
  accountNames: readonly string[]
): string {
  if (accountNames.length > 0) return accountNames.join(" + ")
  return `Import from ${formatDateStamp(createdAt)}`
}
