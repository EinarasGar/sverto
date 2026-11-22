import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

const unlinkPropertyLoan = vi.fn()
const listBindings = vi.fn()

const ENDPOINTS: Record<string, unknown> = { unlinkPropertyLoan, listBindings }

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

const { PropertyLoans } = await import("./property-loans")
const { renderAccounts, stubViewport, TEST_USER_ID } =
  await import("@/features/accounts/test-harness")

const PROPERTY = {
  propertyId: "p-1",
  kind: "real_estate" as const,
  name: "Flat 12",
  accountId: "house",
  assetId: 9,
  currencyAssetId: 1,
  ownershipShare: 1,
  ownershipSharePercent: 100,
  isJoint: false,
  loanAccountIds: ["mortgage"],
}

const LOAN = {
  accountId: "mortgage",
  account: {
    accountId: "mortgage",
    name: "Halifax Mortgage",
    accountTypeId: 7,
    accountTypeName: "Mortgage",
    accountClass: "liabilities" as const,
    isLiquid: false,
    isLiability: true,
    liquidityTypeId: 1,
    liquidityTypeName: "Liquid",
    ownershipShare: 1,
    ownershipSharePercent: 100,
    isJoint: false,
    suggestedCurrencyAssetId: null,
    suggestedCurrency: null,
  },
  balance: -150000,
}

beforeEach(() => {
  vi.clearAllMocks()
  stubViewport(1440)
  unlinkPropertyLoan.mockResolvedValue({ data: undefined })
  listBindings.mockResolvedValue({ data: { bindings: [] } })
})

async function renderLoans() {
  await renderAccounts(
    <PropertyLoans property={PROPERTY} loans={[LOAN]} onLinkLoan={() => {}} />
  )
}

describe("a property's loan rows", () => {
  it("opens the loan's own account page from the whole row", async () => {
    await renderLoans()
    expect(
      screen.getByRole("link", { name: /halifax mortgage/i })
    ).toHaveAttribute("href", "/accounts/mortgage")
  })

  it("asks before unlinking and does nothing on cancel", async () => {
    const user = userEvent.setup()
    await renderLoans()

    await user.click(
      screen.getByRole("button", { name: "Unlink Halifax Mortgage" })
    )
    const dialog = await screen.findByRole("alertdialog")
    expect(dialog).toHaveTextContent("Unlink loan?")
    expect(dialog).toHaveTextContent(
      "Are you sure you want to unlink Halifax Mortgage from Flat 12?"
    )

    await user.click(within(dialog).getByRole("button", { name: "Cancel" }))
    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).toBeNull()
    })
    expect(unlinkPropertyLoan).not.toHaveBeenCalled()
  })

  it("unlinks the loan once confirmed", async () => {
    const user = userEvent.setup()
    await renderLoans()

    await user.click(
      screen.getByRole("button", { name: "Unlink Halifax Mortgage" })
    )
    const dialog = await screen.findByRole("alertdialog")
    await user.click(within(dialog).getByRole("button", { name: "Unlink" }))

    await waitFor(() => {
      expect(unlinkPropertyLoan).toHaveBeenCalledWith(
        "mortgage",
        TEST_USER_ID,
        "p-1"
      )
    })
    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).toBeNull()
    })
  })
})
