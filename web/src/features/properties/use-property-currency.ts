import { useSuspenseQuery } from "@tanstack/react-query"

import { currencyAssetsQueryOptions } from "@/features/onboarding/currency-assets"
import { assetLabel } from "@/lib/domain/refs"

import type { Property } from "./api"

export function usePropertyCurrency(property: Property): string | null {
  const currencies = useSuspenseQuery(currencyAssetsQueryOptions()).data
  const asset = currencies.find(
    (candidate) => candidate.assetId === property.currencyAssetId
  )
  return asset === undefined ? null : assetLabel(asset)
}
