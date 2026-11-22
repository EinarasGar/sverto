import { TransactionsApiFactory } from "@/api"
import { api } from "@/lib/api"
import type { AccountId, AssetId, TransactionId, UserId } from "@/lib/query"
import { apiQueryOptions, queryKeys } from "@/lib/query"

import type { Property } from "./properties"

const PURCHASE_SEARCH_COUNT = 100

export interface PropertyPurchase {
  transactionId: TransactionId
  date: number
}

export async function fetchPropertyPurchase(
  userId: UserId,
  accountId: AccountId,
  assetId: AssetId,
  signal?: AbortSignal
): Promise<PropertyPurchase | null> {
  const response = await api(
    TransactionsApiFactory
  ).getAccountAssetTransactions(
    userId,
    accountId,
    assetId,
    PURCHASE_SEARCH_COUNT,
    0,
    undefined,
    { signal }
  )
  let earliest: PropertyPurchase | null = null
  for (const transaction of response.data.results) {
    if (transaction.type !== "asset_transfer_in") continue
    if (earliest !== null && earliest.date <= transaction.date) continue
    earliest = {
      transactionId: transaction.transaction_id,
      date: transaction.date,
    }
  }
  return earliest
}

export function propertyPurchaseQueryOptions(
  userId: UserId,
  property: Property
) {
  return apiQueryOptions({
    queryKey: queryKeys
      .user(userId)
      .transactions.purchase(property.accountId, property.assetId),
    fetch: ({ signal }) =>
      fetchPropertyPurchase(
        userId,
        property.accountId,
        property.assetId,
        signal
      ),
    meta: { errorContext: "The purchase date could not be loaded" },
  })
}
