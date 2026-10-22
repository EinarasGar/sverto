import { cleanup, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import type { AuthMe } from "@/api"
import { authMeQueryKey } from "@/auth"

const getUsage = vi.fn()
const listBindings = vi.fn()
const getMe = vi.fn()
const postAiFeatures = vi.fn()

const ENDPOINTS: Record<string, unknown> = {
  getUsage,
  listBindings,
  getMe,
  postAiFeatures,
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
  apiClient: { get: vi.fn(() => Promise.resolve({ data: {} })) },
}))

const { SettingsScreen } = await import("./settings-screen")
const {
  createTestQueryClient,
  renderSettings,
  stubViewport,
  TEST_USER_ID,
  VIEWPORTS,
} = await import("./test-harness")
const { MYRA_PERMISSION_UNSTORED } = await import("./copy")

function identity(aiFeaturesEnabled: boolean) {
  return {
    data: {
      user_id: "00000000-0000-0000-0000-000000000000",
      role: "user",
      onboarding_version: 1,
      default_asset: { id: 1, ticker: "GBP", name: "British Pound" },
      user_metadata: { username: "einaras" },
      ai_features_enabled: aiFeaturesEnabled,
    },
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  stubViewport(VIEWPORTS.full)
  getMe.mockResolvedValue(identity(true))
  postAiFeatures.mockResolvedValue({ data: {} })
  listBindings.mockResolvedValue({ data: { bindings: [] } })
  getUsage.mockResolvedValue({
    data: {
      hourly: {
        input: { used: 38_420, limit: 120_000 },
        output: { used: 6_180, limit: 20_000 },
        reset_at: new Date(Date.now() + 24 * 60_000).toISOString(),
      },
      monthly: {
        input: { used: 2_412_880, limit: 4_000_000 },
        output: { used: 412_400, limit: 600_000 },
        reset_at: new Date(Date.now() + 40 * 24 * 3_600_000).toISOString(),
      },
    },
  })
})

afterEach(cleanup)

describe("the Myra quota block", () => {
  it("draws one bar per metric per window, from the real usage endpoint", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    await screen.findByText("Hourly")
    expect(screen.getAllByRole("progressbar")).toHaveLength(4)
  })

  it("prints used, limit and share as separate figures", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    await screen.findByText("Hourly")
    expect(screen.getByText("38,420")).toBeInTheDocument()
    expect(screen.getByText("120,000")).toBeInTheDocument()
    expect(screen.getAllByText("32%").length).toBeGreaterThan(0)
  })

  it("counts down to the reset it was told about", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    expect(await screen.findByText("resets in 24 minutes")).toBeInTheDocument()
  })

  it("escalates the bar tone as a window fills", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    await screen.findByText("Monthly")
    const bars = screen.getAllByRole("progressbar")
    expect(bars.at(-1)?.firstElementChild?.className).toContain("bg-attention")
    expect(bars[0]?.firstElementChild?.className).toContain("bg-brand")
  })
})

describe("what Myra may do", () => {
  it("states the structural rule with no switch at all", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    const row = await screen.findByText("Approval required for every write")
    expect(row).toBeInTheDocument()
    expect(
      screen.getByText(/structural and cannot be switched off/)
    ).toBeVisible()
  })

  it("never lets a placeholder switch look like it persists", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    await screen.findByText("Read receipts I upload")

    const switches = screen.getAllByRole("switch", { name: /placeholder/ })
    expect(switches).toHaveLength(2)
    for (const control of switches) {
      expect(control).toHaveAttribute("aria-disabled", "true")
      expect(control).toHaveAttribute("data-disabled")
    }
    expect(
      screen.getAllByText(new RegExp(MYRA_PERMISSION_UNSTORED.slice(0, 40)))
    ).toHaveLength(2)
  })

  it("marks both placeholder rows so a screenshot stays auditable", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    await screen.findByText("Read receipts I upload")
    await waitFor(() => {
      expect(
        document.querySelectorAll('[data-mock="settings.myra-permissions"]')
          .length
      ).toBeGreaterThanOrEqual(4)
    })
    expect(screen.getAllByText("Not stored")).toHaveLength(2)
  })
})

describe("the Myra quota block while AI features are off", () => {
  it("is not drawn, and usage is never fetched", async () => {
    getMe.mockResolvedValue(identity(false))
    await renderSettings(<SettingsScreen section="myra" />)

    await screen.findByText("einaras")
    expect(
      await screen.findByText("Approval required for every write")
    ).toBeVisible()
    expect(screen.queryByText("Usage")).not.toBeInTheDocument()
    expect(screen.queryByText("Hourly")).not.toBeInTheDocument()
    expect(getUsage).not.toHaveBeenCalled()
  })

  it("appears as soon as AI features are turned on", async () => {
    getMe.mockResolvedValue(identity(false))
    const user = userEvent.setup()
    await renderSettings(<SettingsScreen section="myra" />)
    await screen.findByText("einaras")

    await user.click(await screen.findByRole("switch", { name: "AI features" }))
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: "Turn on",
      })
    )

    expect(await screen.findByText("Hourly")).toBeInTheDocument()
    expect(getUsage).toHaveBeenCalled()
  })

  it("disappears when AI features are turned off", async () => {
    const user = userEvent.setup()
    await renderSettings(<SettingsScreen section="myra" />)
    await screen.findByText("Hourly")

    await user.click(await screen.findByRole("switch", { name: "AI features" }))

    await waitFor(() => {
      expect(screen.queryByText("Hourly")).not.toBeInTheDocument()
    })
  })
})

describe("the AI features switch", () => {
  async function aiSwitch(): Promise<HTMLElement> {
    return screen.findByRole("switch", { name: "AI features" })
  }

  function cachedFlag(
    queryClient: ReturnType<typeof createTestQueryClient>
  ): boolean | undefined {
    return queryClient.getQueryData<AuthMe>(authMeQueryKey)?.ai_features_enabled
  }

  it("states what turning it off means", async () => {
    await renderSettings(<SettingsScreen section="myra" />)
    await aiSwitch()
    expect(
      screen.getByText(
        "Myra chat and receipt scanning use Google's Gemini AI. When off, none of your data is sent to Gemini and these features are unavailable."
      )
    ).toBeVisible()
  })

  it("shows the disclosure before turning on, and saves only on Turn on", async () => {
    getMe.mockResolvedValue(identity(false))
    const user = userEvent.setup()
    await renderSettings(<SettingsScreen section="myra" />)
    const control = await aiSwitch()
    await waitFor(() => {
      expect(control).toHaveAttribute("aria-checked", "false")
    })

    await user.click(control)

    const dialog = await screen.findByRole("alertdialog")
    expect(within(dialog).getByText("Turn on AI features?")).toBeVisible()
    expect(postAiFeatures).not.toHaveBeenCalled()
    expect(control).toHaveAttribute("aria-checked", "false")

    await user.click(within(dialog).getByRole("button", { name: "Turn on" }))

    await waitFor(() => {
      expect(control).toHaveAttribute("aria-checked", "true")
    })
    expect(postAiFeatures).toHaveBeenCalledWith(TEST_USER_ID, {
      enabled: true,
    })
  })

  it("stays off when the answer is Not now", async () => {
    getMe.mockResolvedValue(identity(false))
    const user = userEvent.setup()
    await renderSettings(<SettingsScreen section="myra" />)
    const control = await aiSwitch()

    await user.click(control)
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: "Not now",
      })
    )

    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument()
    })
    expect(control).toHaveAttribute("aria-checked", "false")
    expect(postAiFeatures).not.toHaveBeenCalled()
  })

  it("turns off straight away, with no confirmation, and updates the identity", async () => {
    const queryClient = createTestQueryClient()
    const user = userEvent.setup()
    await renderSettings(<SettingsScreen section="myra" />, queryClient)
    const control = await aiSwitch()
    await waitFor(() => {
      expect(control).toHaveAttribute("aria-checked", "true")
    })

    await user.click(control)

    await waitFor(() => {
      expect(postAiFeatures).toHaveBeenCalledWith(TEST_USER_ID, {
        enabled: false,
      })
    })
    await waitFor(() => {
      expect(cachedFlag(queryClient)).toBe(false)
    })
    expect(control).toHaveAttribute("aria-checked", "false")
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument()
  })

  it("locks while saving and springs back on when turning off fails", async () => {
    let fail: (reason: unknown) => void = () => {}
    postAiFeatures.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          fail = reject
        })
    )
    const queryClient = createTestQueryClient()
    const user = userEvent.setup()
    await renderSettings(<SettingsScreen section="myra" />, queryClient)
    const control = await aiSwitch()
    await waitFor(() => {
      expect(control).toHaveAttribute("aria-checked", "true")
    })

    await user.click(control)

    await waitFor(() => {
      expect(control).toHaveAttribute("data-disabled")
    })
    expect(control).toHaveAttribute("aria-checked", "false")

    fail(new Error("boom"))

    await waitFor(() => {
      expect(control).not.toHaveAttribute("data-disabled")
    })
    expect(control).toHaveAttribute("aria-checked", "true")
    expect(cachedFlag(queryClient)).toBe(true)
  })
})
