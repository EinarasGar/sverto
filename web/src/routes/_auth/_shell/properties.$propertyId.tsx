import { createFileRoute } from "@tanstack/react-router"

import {
  RouteErrorPanel,
  RoutePending,
} from "@/components/layout/route-boundaries"
import {
  accountConnectorsQueryOptions,
  accountsQueryOptions,
} from "@/features/accounts/api"
import {
  userAssetConvertedRatesQueryOptions,
  userAssetPairRatesQueryOptions,
} from "@/features/assets/api"
import { currencyAssetsQueryOptions } from "@/features/onboarding/currency-assets"
import { holdingsQueryOptions } from "@/features/portfolio/api"
import { PropertyDetail } from "@/features/properties"
import { propertiesQueryOptions } from "@/features/properties/api"
import { warm } from "@/lib/query"

import { warmScope } from "../../-warm"

function PropertyDetailRoute() {
  const { propertyId } = Route.useParams()
  return <PropertyDetail propertyId={propertyId} />
}

export const Route = createFileRoute("/_auth/_shell/properties/$propertyId")({
  loader: async ({ context, params }) => {
    const scope = await warmScope(context)
    if (scope === null) return
    const { queryClient, userId, defaultAssetId } = scope
    const properties = queryClient.ensureQueryData(
      propertiesQueryOptions(userId)
    )
    warm([
      properties,
      queryClient.ensureQueryData(accountsQueryOptions(userId)),
      queryClient.ensureQueryData(currencyAssetsQueryOptions()),
      queryClient.ensureQueryData(accountConnectorsQueryOptions(userId)),
      queryClient.ensureQueryData(
        holdingsQueryOptions({ userId, defaultAssetId })
      ),
      queryClient.ensureQueryData(
        holdingsQueryOptions({
          userId,
          defaultAssetId,
          applyOwnershipShare: false,
        })
      ),
      properties.then((view) => {
        const property = view.byId.get(params.propertyId)
        if (property === undefined) return
        return property.currencyAssetId === defaultAssetId
          ? queryClient.ensureQueryData(
              userAssetPairRatesQueryOptions(
                userId,
                property.assetId,
                property.currencyAssetId,
                "all"
              )
            )
          : queryClient.ensureQueryData(
              userAssetConvertedRatesQueryOptions(
                userId,
                property.assetId,
                defaultAssetId,
                "all"
              )
            )
      }),
    ])
  },
  component: PropertyDetailRoute,
  errorComponent: RouteErrorPanel,
  pendingComponent: RoutePending,
})
