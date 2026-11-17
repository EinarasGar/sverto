import { createFileRoute } from "@tanstack/react-router"

import {
  RouteErrorPanel,
  RoutePending,
} from "@/components/layout/route-boundaries"
import { ImportSessionDetail } from "@/features/imports"
import { accountsQueryOptions } from "@/features/accounts/api"
import { importSessionQueryOptions } from "@/features/imports/api"
import { warm } from "@/lib/query"

import { warmScope } from "../../-warm"

function ImportSessionRoute() {
  const { sessionId } = Route.useParams()
  return <ImportSessionDetail sessionId={sessionId} />
}

export const Route = createFileRoute("/_auth/_shell/imports/$sessionId")({
  loader: async ({ context, params }) => {
    const scope = await warmScope(context)
    if (scope === null) return
    const { queryClient, userId } = scope
    warm([
      queryClient.ensureQueryData(
        importSessionQueryOptions({ userId, sessionId: params.sessionId })
      ),
      queryClient.ensureQueryData(accountsQueryOptions(userId)),
    ])
  },
  component: ImportSessionRoute,
  errorComponent: RouteErrorPanel,
  pendingComponent: RoutePending,
})
