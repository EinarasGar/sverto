import { createFileRoute, notFound } from "@tanstack/react-router"
import { z } from "zod"

import { CHART_PERIODS } from "@/components/chart"

import {
  RouteErrorPanel,
  RoutePending,
} from "@/components/layout/route-boundaries"
import {
  assetTypesQueryOptions,
  userAssetsQueryOptions,
} from "@/features/assets/api"
import { ASSET_CHART_SERIES, AssetScreen } from "@/features/portfolio"
import {
  assetOverviewQueryOptions,
  holdingsQueryOptions,
} from "@/features/portfolio/api"
import { propertiesQueryOptions } from "@/features/properties/api"
import { warm } from "@/lib/query"

import { optionalEnum, optionalText } from "../../-search"
import { warmScope } from "../../-warm"

const assetSearchSchema = z.object({
  period: optionalEnum(CHART_PERIODS),
  chart: optionalEnum(ASSET_CHART_SERIES),
  rows: optionalText,
  account: optionalText,
})

function AssetRoute() {
  const { assetId } = Route.useParams()
  const { account } = Route.useSearch()
  const parsed = Number(assetId)
  if (!Number.isInteger(parsed)) throw notFound()
  return <AssetScreen assetId={parsed} accountId={account} />
}

export const Route = createFileRoute("/_auth/_shell/portfolio/$assetId")({
  validateSearch: assetSearchSchema,
  loaderDeps: ({ search }) => ({ account: search.account }),
  loader: async ({ context, deps, params }) => {
    const assetId = Number(params.assetId)
    if (!Number.isInteger(assetId)) return
    const scope = await warmScope(context)
    if (scope === null) return
    const { queryClient, userId, defaultAssetId } = scope
    warm([
      queryClient.ensureQueryData(
        assetOverviewQueryOptions({ userId, assetId, defaultAssetId })
      ),
      queryClient.ensureQueryData(
        holdingsQueryOptions({
          userId,
          defaultAssetId,
          includeZeroBalances: deps.account !== undefined,
        })
      ),
      queryClient.ensureQueryData(propertiesQueryOptions(userId)),
      queryClient.ensureQueryData(userAssetsQueryOptions(userId)),
      queryClient.ensureQueryData(assetTypesQueryOptions()),
    ])
  },
  component: AssetRoute,
  errorComponent: RouteErrorPanel,
  pendingComponent: RoutePending,
})
