import { useSuspenseQuery } from "@tanstack/react-query"

import type { GetPropertiesResponse, PropertyKind } from "@/api"
import { PropertiesApiFactory } from "@/api"
import { api } from "@/lib/api"
import { isJointAccount, ownershipSharePercent } from "@/lib/domain/accounts"
import type { UserId } from "@/lib/query"
import { apiQueryOptions, queryKeys, STALE_TIMES } from "@/lib/query"

export interface Property {
  propertyId: string
  kind: PropertyKind
  name: string
  accountId: string
  assetId: number
  currencyAssetId: number
  ownershipShare: number
  ownershipSharePercent: number
  isJoint: boolean
  loanAccountIds: string[]
}

export interface PropertiesView {
  properties: Property[]
  byId: ReadonlyMap<string, Property>
  byAccountId: ReadonlyMap<string, Property>
  byLoanAccountId: ReadonlyMap<string, Property>
}

export function buildPropertiesView(
  response: GetPropertiesResponse
): PropertiesView {
  const properties = response.properties
    .map((row): Property => ({
      propertyId: row.property_id,
      kind: row.kind,
      name: row.name,
      accountId: row.account_id,
      assetId: row.asset_id,
      currencyAssetId: row.currency_asset_id,
      ownershipShare: row.ownership_share,
      ownershipSharePercent: ownershipSharePercent(row.ownership_share),
      isJoint: isJointAccount(row.ownership_share),
      loanAccountIds: row.loan_account_ids,
    }))
    .sort((a, b) => a.name.localeCompare(b.name))

  return {
    properties,
    byId: new Map(
      properties.map((property) => [property.propertyId, property])
    ),
    byAccountId: new Map(
      properties.map((property) => [property.accountId, property])
    ),
    byLoanAccountId: new Map(
      properties.flatMap((property) =>
        property.loanAccountIds.map((loanId) => [loanId, property] as const)
      )
    ),
  }
}

export function propertiesQueryOptions(userId: UserId) {
  return apiQueryOptions({
    queryKey: queryKeys.user(userId).properties.list(),
    staleTime: STALE_TIMES.standard,
    fetch: async ({ signal }): Promise<PropertiesView> => {
      const response = await api(PropertiesApiFactory).getProperties(userId, {
        signal,
      })
      return buildPropertiesView(response.data)
    },
    meta: { errorContext: "Properties could not be loaded" },
  })
}

export function usePropertiesSuspense(userId: UserId): PropertiesView {
  return useSuspenseQuery(propertiesQueryOptions(userId)).data
}
