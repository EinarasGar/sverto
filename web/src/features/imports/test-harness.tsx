import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
  useParams,
  type AnyRouter,
  type RegisteredRouter,
} from "@tanstack/react-router"
import { render } from "@testing-library/react"

import { AuthSessionContext, type AuthSession } from "@/auth"
import { seedAuthMe } from "@/auth/test-harness"
import { PanelBoundary } from "@/components/layout/route-boundaries"
import { Toaster } from "@/components/ui/toast"

import { ImportSessionDetail } from "./import-session-detail"
import { ImportsIndex } from "./imports-index"

export const TEST_USER_ID = "00000000-0000-0000-0000-000000000000"

const SESSION: AuthSession = {
  status: "authenticated",
  isReady: true,
  isAuthenticated: true,
  userId: TEST_USER_ID,
  baseCurrency: "GBP",
  signOut: () => Promise.resolve(),
}

export function renderImports(
  initialPath: string,
  { aiFeaturesEnabled = true }: { aiFeaturesEnabled?: boolean } = {}
) {
  function DetailRoute() {
    const { sessionId } = useParams({ strict: false })
    if (sessionId === undefined) return null
    return <ImportSessionDetail sessionId={sessionId} />
  }

  const queryClient = seedAuthMe(
    new QueryClient({
      defaultOptions: {
        queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
      },
    }),
    { ai_features_enabled: aiFeaturesEnabled }
  )
  const rootRoute = createRootRoute({
    component: () => (
      <QueryClientProvider client={queryClient}>
        <AuthSessionContext.Provider value={SESSION}>
          <Toaster>
            <PanelBoundary>
              <Outlet />
            </PanelBoundary>
          </Toaster>
        </AuthSessionContext.Provider>
      </QueryClientProvider>
    ),
  })
  const routeTree = rootRoute.addChildren([
    createRoute({
      getParentRoute: () => rootRoute,
      path: "/imports",
      component: ImportsIndex,
    }),
    createRoute({
      getParentRoute: () => rootRoute,
      path: "/imports/$sessionId",
      component: DetailRoute,
    }),
  ])
  const router: AnyRouter = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })
  render(<RouterProvider router={router as unknown as RegisteredRouter} />)
  return { router, queryClient }
}
