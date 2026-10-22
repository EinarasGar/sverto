import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

import type { AuthMe } from "@/api"
import { authMeQueryKey } from "@/auth"

const searchAssets = vi.fn()
const postBaseAsset = vi.fn()
const postAiFeatures = vi.fn()
const postOnboarding = vi.fn()

const ENDPOINTS: Record<string, unknown> = {
  searchAssets,
  postBaseAsset,
  postAiFeatures,
  postOnboarding,
}

vi.mock("@/lib/api", () => ({
  api: () =>
    new Proxy(
      {},
      {
        get: (_target, name: string) =>
          ENDPOINTS[name] ?? (() => Promise.resolve({ data: {} })),
      }
    ),
  resolveApiUrl: (path: string) => `http://localhost:5000${path}`,
}))

const { OnboardingWizard } = await import("./wizard")
const { seedAuthMe } = await import("@/auth/test-harness")
const {
  authenticatedSession,
  createTestQueryClient,
  renderInShell,
  stubViewport,
  TEST_USER_ID,
  VIEWPORTS,
} = await import("./test-harness")

const CURRENCIES = [
  { asset_id: 1, ticker: "EUR", name: "Euro", asset_type: 1 },
  { asset_id: 2, ticker: "GBP", name: "British Pound Sterling", asset_type: 1 },
  { asset_id: 3, ticker: "USD", name: "United States Dollar", asset_type: 1 },
]

beforeEach(() => {
  vi.clearAllMocks()
  stubViewport(VIEWPORTS.full)
  searchAssets.mockResolvedValue({ data: { results: CURRENCIES } })
  postBaseAsset.mockResolvedValue({ data: {} })
  postAiFeatures.mockResolvedValue({ data: {} })
  postOnboarding.mockResolvedValue({ data: {} })
})

async function stepToCurrency(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: /continue/i }))
  await screen.findByRole("listbox", { name: /search currencies/i })
}

async function keepAiOff(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "Keep off" }))
  await screen.findByText("Step 4 of 4")
}

describe("OnboardingWizard", () => {
  it("walks welcome to base currency to AI features to start, and only then finishes onboarding", async () => {
    const user = userEvent.setup()
    const { visited, queryClient } = await renderInShell(<OnboardingWizard />, {
      queryClient: seedAuthMe(createTestQueryClient()),
    })

    expect(screen.getByText("Step 1 of 4")).toBeInTheDocument()
    expect(screen.getByText("One ledger for all of it")).toBeInTheDocument()

    await stepToCurrency(user)
    expect(screen.getByText("Step 2 of 4")).toBeInTheDocument()
    expect(postBaseAsset).not.toHaveBeenCalled()

    await user.click(screen.getByRole("option", { name: /GBP/ }))
    await user.click(screen.getByRole("button", { name: /Use GBP/ }))

    await waitFor(() =>
      expect(postBaseAsset).toHaveBeenCalledWith(TEST_USER_ID, { asset_id: 2 })
    )

    await screen.findByText("Step 3 of 4")
    expect(screen.getByRole("heading", { name: "AI features" })).toBeVisible()
    expect(postAiFeatures).not.toHaveBeenCalled()
    await user.click(
      screen.getByRole("button", { name: /Turn on AI features/ })
    )

    await waitFor(() =>
      expect(postAiFeatures).toHaveBeenCalledWith(TEST_USER_ID, {
        enabled: true,
      })
    )
    expect(
      queryClient.getQueryData<AuthMe>(authMeQueryKey)?.ai_features_enabled
    ).toBe(true)
    expect(postOnboarding).not.toHaveBeenCalled()

    await screen.findByText("Step 4 of 4")
    await user.click(screen.getByRole("button", { name: /Add a transaction/ }))

    await waitFor(() =>
      expect(postOnboarding).toHaveBeenCalledWith(TEST_USER_ID, { version: 2 })
    )
    await waitFor(() => expect(visited).toContain("/transactions"))
  })

  it("resumes at the step after the base currency when one already exists", async () => {
    await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
    })

    expect(await screen.findByText("Step 3 of 4")).toBeInTheDocument()
    expect(screen.getByRole("heading", { name: "AI features" })).toBeVisible()
    expect(screen.getByText(/^GBP · Saved\./)).toBeInTheDocument()
  })

  it("discloses what Google receives before asking, with the privacy policy one click away", async () => {
    await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
    })

    await screen.findByText("Step 3 of 4")
    expect(
      screen.getByText(
        "Myra chat and receipt scanning use Google's Gemini AI. To use them, Sverto sends Google:"
      )
    ).toBeVisible()
    expect(
      screen.getByText("Your transaction descriptions, so Myra can search them")
    ).toBeVisible()
    expect(
      screen.getByRole("link", { name: /Privacy policy/ })
    ).toHaveAttribute("href", "https://sverto.com/privacy")
  })

  it("records a refusal as AI features off and still moves on", async () => {
    const user = userEvent.setup()
    await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
    })

    await keepAiOff(user)

    expect(postAiFeatures).toHaveBeenCalledWith(TEST_USER_ID, {
      enabled: false,
    })
    expect(postOnboarding).not.toHaveBeenCalled()
  })

  it("stays on the AI step when the choice cannot be saved", async () => {
    postAiFeatures.mockRejectedValue(new Error("boom"))
    const user = userEvent.setup()
    await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
    })

    await user.click(await screen.findByRole("button", { name: "Keep off" }))

    expect(await screen.findByRole("alert")).toBeInTheDocument()
    expect(screen.getByText("Step 3 of 4")).toBeInTheDocument()
    expect(postOnboarding).not.toHaveBeenCalled()
  })

  it("shows an account that finished version 1 only the AI step, then finishes", async () => {
    const user = userEvent.setup()
    const { visited } = await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
      queryClient: seedAuthMe(createTestQueryClient(), {
        onboarding_version: 1,
        default_asset: { id: 2, ticker: "GBP" },
      }),
    })

    expect(await screen.findByText("Step 1 of 1")).toBeInTheDocument()
    const tracker = screen.getByRole("list", { name: "Setup steps" })
    expect(within(tracker).getAllByRole("listitem")).toHaveLength(1)
    expect(within(tracker).getByText("AI features")).toBeInTheDocument()
    expect(screen.queryByRole("button", { name: "Back" })).toBeNull()
    expect(screen.queryByText(/^GBP · Saved\./)).toBeNull()

    await user.click(
      screen.getByRole("button", { name: /Turn on AI features/ })
    )

    await waitFor(() =>
      expect(postOnboarding).toHaveBeenCalledWith(TEST_USER_ID, { version: 2 })
    )
    expect(postAiFeatures).toHaveBeenCalledWith(TEST_USER_ID, {
      enabled: true,
    })
    await waitFor(() => expect(visited).toContain("/"))
  })

  it.each([
    ["bank", /Connect a bank/, "/settings?section=connections"],
    ["receipt", /Snap a receipt/, "/transactions?mode=review"],
  ])("sends %s to %s", async (_id, label, destination) => {
    const user = userEvent.setup()
    const { visited } = await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
    })

    await keepAiOff(user)
    await user.click(await screen.findByRole("button", { name: label }))
    await waitFor(() => expect(visited).toContain(destination))
  })

  it("advances onboarding before skipping to the dashboard", async () => {
    const user = userEvent.setup()
    const { visited } = await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
    })

    await keepAiOff(user)
    await user.click(
      await screen.findByRole("button", { name: /Skip for now/ })
    )

    await waitFor(() =>
      expect(postOnboarding).toHaveBeenCalledWith(TEST_USER_ID, { version: 2 })
    )
    await waitFor(() => expect(visited).toContain("/"))
  })

  it("never navigates away when finishing onboarding fails", async () => {
    postOnboarding.mockRejectedValue(new Error("boom"))
    const user = userEvent.setup()
    const { visited } = await renderInShell(<OnboardingWizard />, {
      session: authenticatedSession("GBP"),
    })

    await keepAiOff(user)
    await user.click(
      await screen.findByRole("button", { name: /Skip for now/ })
    )

    expect(await screen.findByRole("alert")).toBeInTheDocument()
    expect(visited).not.toContain("/")
  })

  it.each([
    ["full", VIEWPORTS.full, true],
    ["tight", VIEWPORTS.tight, true],
    ["stacked", VIEWPORTS.stacked, false],
    ["phone", VIEWPORTS.phone, false],
  ])(
    "stays operable at %s, with the key hint only where a keyboard is likely",
    async (_name, width, hasKeyHint) => {
      stubViewport(width)
      await renderInShell(<OnboardingWizard />)

      expect(screen.getByText("Step 1 of 4")).toBeInTheDocument()
      const next = screen.getByRole("button", { name: /Continue/ })
      expect(next).toBeEnabled()
      expect(next.textContent?.includes("⏎")).toBe(hasKeyHint)

      const tracker = screen.getByRole("list", { name: "Setup steps" })
      expect(within(tracker).getByText("Base currency")).toBeInTheDocument()
    }
  )
})
