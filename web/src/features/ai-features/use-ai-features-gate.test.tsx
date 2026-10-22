import { useState } from "react"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

import type { AuthMe } from "@/api"
import { authMeQueryKey } from "@/auth"
import { seedAuthMe } from "@/auth/test-harness"

const postAiFeatures = vi.fn()

vi.mock("@/lib/api", () => ({
  api: () => ({ postAiFeatures }),
  resolveApiUrl: (path: string) => `http://localhost:5000${path}`,
}))

const { useAiFeaturesGate } = await import("./use-ai-features-gate")

const TEST_USER_ID = "00000000-0000-0000-0000-000000000000"

const AI_FEATURES_DISABLED = {
  kind: "forbidden",
  message: "AI features are disabled for this account.",
  errorType: "AiFeaturesDisabled",
}

function Harness({
  action,
  required,
}: {
  action: () => unknown
  required?: () => Promise<string>
}) {
  const gate = useAiFeaturesGate(TEST_USER_ID)
  const [outcome, setOutcome] = useState("")
  return (
    <>
      <button
        type="button"
        onClick={() => {
          gate.runWithAiConsent(action)
        }}
      >
        Ask Myra
      </button>
      {required === undefined ? null : (
        <button
          type="button"
          onClick={() => {
            gate.requireAiConsent(required).then(setOutcome, () => {
              setOutcome("declined")
            })
          }}
        >
          Hand to Myra
        </button>
      )}
      <output>{outcome}</output>
      {gate.dialog}
    </>
  )
}

function renderGate(
  aiFeaturesEnabled: boolean,
  action: () => unknown = vi.fn(),
  required?: () => Promise<string>
) {
  const queryClient = seedAuthMe(
    new QueryClient({
      defaultOptions: {
        queries: { retry: false, staleTime: Infinity },
        mutations: { retry: false },
      },
    }),
    { ai_features_enabled: aiFeaturesEnabled }
  )
  render(
    <QueryClientProvider client={queryClient}>
      <Harness
        action={action}
        {...(required === undefined ? {} : { required })}
      />
    </QueryClientProvider>
  )
  return { queryClient, action }
}

function cachedFlag(queryClient: QueryClient): boolean | undefined {
  return queryClient.getQueryData<AuthMe>(authMeQueryKey)?.ai_features_enabled
}

beforeEach(() => {
  postAiFeatures.mockReset()
  postAiFeatures.mockResolvedValue({ data: {} })
})

describe("useAiFeaturesGate", () => {
  it("runs the action straight away when AI features are on", async () => {
    const user = userEvent.setup()
    const { action } = renderGate(true)

    await user.click(screen.getByRole("button", { name: "Ask Myra" }))

    expect(action).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole("alertdialog")).toBeNull()
    expect(postAiFeatures).not.toHaveBeenCalled()
  })

  it("asks first when AI features are off, and says what Google receives", async () => {
    const user = userEvent.setup()
    const { action } = renderGate(false)

    await user.click(screen.getByRole("button", { name: "Ask Myra" }))

    const dialog = await screen.findByRole("alertdialog")
    expect(within(dialog).getByText("Turn on AI features?")).toBeInTheDocument()
    expect(
      within(dialog).getByText(
        "Your Myra messages and any images or receipts you attach"
      )
    ).toBeInTheDocument()
    expect(
      within(dialog).getByRole("link", { name: /Privacy policy/ })
    ).toHaveAttribute("target", "_blank")
    expect(action).not.toHaveBeenCalled()
  })

  it("turns AI features on, then carries on with what was attempted", async () => {
    const user = userEvent.setup()
    const { action, queryClient } = renderGate(false)

    await user.click(screen.getByRole("button", { name: "Ask Myra" }))
    await user.click(await screen.findByRole("button", { name: "Turn on" }))

    await waitFor(() => {
      expect(action).toHaveBeenCalledTimes(1)
    })
    expect(postAiFeatures).toHaveBeenCalledWith(TEST_USER_ID, {
      enabled: true,
    })
    expect(cachedFlag(queryClient)).toBe(true)
    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).toBeNull()
    })
  })

  it("does nothing on Not now, and asks again on the next attempt", async () => {
    const user = userEvent.setup()
    const { action, queryClient } = renderGate(false)

    await user.click(screen.getByRole("button", { name: "Ask Myra" }))
    await user.click(await screen.findByRole("button", { name: "Not now" }))

    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).toBeNull()
    })
    expect(action).not.toHaveBeenCalled()
    expect(postAiFeatures).not.toHaveBeenCalled()
    expect(cachedFlag(queryClient)).toBe(false)

    await user.click(screen.getByRole("button", { name: "Ask Myra" }))
    expect(await screen.findByRole("alertdialog")).toBeInTheDocument()
  })

  it("keeps asking and never carries on when turning AI features on fails", async () => {
    postAiFeatures.mockRejectedValue(new Error("boom"))
    const user = userEvent.setup()
    const { action, queryClient } = renderGate(false)

    await user.click(screen.getByRole("button", { name: "Ask Myra" }))
    await user.click(await screen.findByRole("button", { name: "Turn on" }))

    const dialog = await screen.findByRole("alertdialog")
    expect(await within(dialog).findByRole("alert")).toBeInTheDocument()
    expect(action).not.toHaveBeenCalled()
    expect(cachedFlag(queryClient)).toBe(false)
  })

  it("asks again when the server says AI features are off after all, then retries", async () => {
    const action = vi
      .fn()
      .mockRejectedValueOnce(AI_FEATURES_DISABLED)
      .mockResolvedValueOnce(undefined)
    const user = userEvent.setup()
    const { queryClient } = renderGate(true, action)

    await user.click(screen.getByRole("button", { name: "Ask Myra" }))

    expect(await screen.findByRole("alertdialog")).toBeInTheDocument()
    expect(cachedFlag(queryClient)).toBe(false)
    expect(action).toHaveBeenCalledTimes(1)

    await user.click(screen.getByRole("button", { name: "Turn on" }))

    await waitFor(() => {
      expect(action).toHaveBeenCalledTimes(2)
    })
    expect(cachedFlag(queryClient)).toBe(true)
  })

  it("resolves a required action with its result once consent is given", async () => {
    const user = userEvent.setup()
    renderGate(false, vi.fn(), () => Promise.resolve("handed over"))

    await user.click(screen.getByRole("button", { name: "Hand to Myra" }))
    await user.click(await screen.findByRole("button", { name: "Turn on" }))

    expect(await screen.findByText("handed over")).toBeInTheDocument()
  })

  it("rejects a required action when consent is declined", async () => {
    const required = vi.fn(() => Promise.resolve("handed over"))
    const user = userEvent.setup()
    renderGate(false, vi.fn(), required)

    await user.click(screen.getByRole("button", { name: "Hand to Myra" }))
    await user.click(await screen.findByRole("button", { name: "Not now" }))

    expect(await screen.findByText("declined")).toBeInTheDocument()
    expect(required).not.toHaveBeenCalled()
  })
})
