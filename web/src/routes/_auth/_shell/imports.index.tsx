import { createFileRoute } from "@tanstack/react-router"

import {
  RouteErrorPanel,
  RoutePending,
} from "@/components/layout/route-boundaries"
import { ImportsIndex } from "@/features/imports"
import { accountsQueryOptions } from "@/features/accounts/api"
import { importSessionsQueryOptions } from "@/features/imports/api"
import { warm } from "@/lib/query"

import { warmScope } from "../../-warm"

export const Route = createFileRoute("/_auth/_shell/imports/")({
  loader: async ({ context }) => {
    const scope = await warmScope(context)
    if (scope === null) return
    const { queryClient, userId } = scope
    warm([
      queryClient.ensureQueryData(importSessionsQueryOptions(userId)),
      queryClient.ensureQueryData(accountsQueryOptions(userId)),
    ])
  },
  component: ImportsIndex,
  errorComponent: RouteErrorPanel,
  pendingComponent: RoutePending,
})
