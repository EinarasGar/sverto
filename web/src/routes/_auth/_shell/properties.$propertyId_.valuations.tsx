import { createFileRoute } from "@tanstack/react-router"

import {
  RouteErrorPanel,
  RoutePending,
} from "@/components/layout/route-boundaries"
import { userAssetPairRatesQueryOptions } from "@/features/assets/api"
import { currencyAssetsQueryOptions } from "@/features/onboarding/currency-assets"
import { PropertyValuations } from "@/features/properties"
import { propertiesQueryOptions } from "@/features/properties/api"
import { warm } from "@/lib/query"

import { warmScope } from "../../-warm"

function PropertyValuationsRoute() {
  const { propertyId } = Route.useParams()
  return <PropertyValuations propertyId={propertyId} />
}

export const Route = createFileRoute(
  "/_auth/_shell/properties/$propertyId_/valuations"
)({
  loader: async ({ context, params }) => {
    const scope = await warmScope(context)
    if (scope === null) return
    const { queryClient, userId } = scope
    const properties = queryClient.ensureQueryData(
      propertiesQueryOptions(userId)
    )
    warm([
      properties,
      queryClient.ensureQueryData(currencyAssetsQueryOptions()),
      properties.then((view) => {
        const property = view.byId.get(params.propertyId)
        if (property === undefined) return
        return queryClient.ensureQueryData(
          userAssetPairRatesQueryOptions(
            userId,
            property.assetId,
            property.currencyAssetId,
            "all"
          )
        )
      }),
    ])
  },
  component: PropertyValuationsRoute,
  errorComponent: RouteErrorPanel,
  pendingComponent: RoutePending,
})
