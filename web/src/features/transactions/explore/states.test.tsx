import type { ReactNode } from "react"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { AuthSessionContext, type AuthSession } from "@/auth"
import { seedAuthMe } from "@/auth/test-harness"
import { renderInRouter } from "@/features/dashboard/test-router"

import { LEDGER_EMPTY_BODY } from "./copy"
import { LedgerEmpty } from "./states"

const SESSION: AuthSession = {
  status: "authenticated",
  isReady: true,
  isAuthenticated: true,
  userId: "00000000-0000-0000-0000-000000000000",
  baseCurrency: "GBP",
  signOut: () => Promise.resolve(),
}

function renderSignedIn(node: ReactNode, aiFeaturesEnabled = true) {
  const queryClient = seedAuthMe(new QueryClient(), {
    ai_features_enabled: aiFeaturesEnabled,
  })
  return renderInRouter(
    <QueryClientProvider client={queryClient}>
      <AuthSessionContext.Provider value={SESSION}>
        {node}
      </AuthSessionContext.Provider>
    </QueryClientProvider>
  )
}

function buttonNames() {
  return screen.getAllByRole("button").map((button) => button.textContent)
}

describe("LedgerEmpty", () => {
  it("offers every route its own copy promises", async () => {
    await renderSignedIn(<LedgerEmpty onAdd={vi.fn()} onConnect={vi.fn()} />)

    expect(screen.getByText(LEDGER_EMPTY_BODY)).toBeVisible()
    expect(buttonNames()).toEqual([
      "Add a transaction",
      "Connect a bank",
      "Snap a receipt",
    ])
  })

  it("sends the receipt route to the screen that actually takes uploads", async () => {
    const user = userEvent.setup()
    const { router } = await renderSignedIn(
      <LedgerEmpty onAdd={vi.fn()} onConnect={vi.fn()} />
    )

    await user.click(screen.getByRole("button", { name: "Snap a receipt" }))

    expect(router.state.location.pathname).toBe("/transactions")
    expect(router.state.location.search).toMatchObject({
      mode: "review",
      upload: "receipt",
    })
  })

  it("asks to turn AI features on before opening the receipt uploader", async () => {
    const user = userEvent.setup()
    const { router } = await renderSignedIn(
      <LedgerEmpty onAdd={vi.fn()} onConnect={vi.fn()} />,
      false
    )

    await user.click(screen.getByRole("button", { name: "Snap a receipt" }))

    expect(await screen.findByRole("alertdialog")).toBeInTheDocument()
    expect(router.state.location.pathname).toBe("/")
  })
})
