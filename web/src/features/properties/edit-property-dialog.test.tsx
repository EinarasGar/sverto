import { screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

const getAccountAssetTransactions = vi.fn()
const updateProperty = vi.fn()
const updateAnExistingIndividualTransaction = vi.fn()
const getUserAssetPairRates = vi.fn()
const postCustomAssetRates = vi.fn()
const deleteAssetPairRates = vi.fn()
const getTransaction = vi.fn()

const ENDPOINTS: Record<string, unknown> = {
  getAccountAssetTransactions,
  updateProperty,
  updateAnExistingIndividualTransaction,
  getUserAssetPairRates,
  postCustomAssetRates,
  deleteAssetPairRates,
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
  apiClient: { get: getTransaction },
}))

const { EditPropertyDialog } = await import("./edit-property-dialog")
const { renderAccounts, stubViewport, TEST_USER_ID } =
  await import("@/features/accounts/test-harness")

const BOUGHT = Date.UTC(2024, 4, 1, 12) / 1000
const TRANSFER = {
  type: "asset_transfer_in",
  date: BOUGHT,
  fees: null,
  entry: { entry_id: 16, account_id: "car", asset_id: 9, amount: 1 },
}

const PROPERTY = {
  propertyId: "p-1",
  kind: "vehicle" as const,
  name: "Web Car",
  accountId: "car",
  assetId: 9,
  currencyAssetId: 45,
  ownershipShare: 1,
  ownershipSharePercent: 100,
  isJoint: false,
  loanAccountIds: [],
}

const onOpenChange = vi.fn()

beforeEach(() => {
  vi.clearAllMocks()
  stubViewport(1440)
  getAccountAssetTransactions.mockResolvedValue({
    data: {
      results: [{ ...TRANSFER, transaction_id: "bought" }],
      total_results: 1,
      lookup_tables: { accounts: [], assets: [] },
    },
  })
  getTransaction.mockResolvedValue({
    data: { transaction: TRANSFER, lookup_tables: {} },
  })
  updateProperty.mockResolvedValue({ data: {} })
  updateAnExistingIndividualTransaction.mockResolvedValue({ data: {} })
  getUserAssetPairRates.mockResolvedValue({
    data: { range: "all", rates: [{ date: BOUGHT, rate: 25000 }] },
  })
  postCustomAssetRates.mockResolvedValue({ data: undefined })
  deleteAssetPairRates.mockResolvedValue({ data: undefined })
})

async function renderDialog() {
  await renderAccounts(
    <EditPropertyDialog property={PROPERTY} onOpenChange={onOpenChange} />
  )
  const boughtOn = await screen.findByLabelText("Bought on")
  await waitFor(() => {
    expect(boughtOn).toHaveValue("1 May 2024")
  })
  return boughtOn
}

describe("EditPropertyDialog bought on", () => {
  it("moves the purchase transfer and its valuation after saving the name", async () => {
    const user = userEvent.setup()
    const boughtOn = await renderDialog()

    await user.clear(boughtOn)
    await user.type(boughtOn, "2023-03-01")
    await user.click(screen.getByRole("button", { name: "Save changes" }))

    const moved = new Date(2023, 2, 1).getTime() / 1000
    await waitFor(() => {
      expect(onOpenChange).toHaveBeenCalledWith(false)
    })
    expect(updateProperty).toHaveBeenCalledWith(TEST_USER_ID, "p-1", {
      name: "Web Car",
      ownership_share: 1,
    })
    expect(updateAnExistingIndividualTransaction).toHaveBeenCalledWith(
      TEST_USER_ID,
      "bought",
      { transaction: { ...TRANSFER, date: moved } }
    )
    expect(postCustomAssetRates).toHaveBeenCalledWith(TEST_USER_ID, 9, 45, {
      rates: [{ date: moved, rate: 25000 }],
    })
    expect(deleteAssetPairRates).toHaveBeenCalledWith(
      TEST_USER_ID,
      9,
      45,
      BOUGHT,
      BOUGHT + 1
    )
  })

  it("leaves the purchase alone when the day is unchanged", async () => {
    const user = userEvent.setup()
    await renderDialog()

    await user.click(screen.getByRole("button", { name: "Save changes" }))

    await waitFor(() => {
      expect(onOpenChange).toHaveBeenCalledWith(false)
    })
    expect(updateProperty).toHaveBeenCalled()
    expect(updateAnExistingIndividualTransaction).not.toHaveBeenCalled()
    expect(postCustomAssetRates).not.toHaveBeenCalled()
  })

  it("refuses a future date before saving anything", async () => {
    const user = userEvent.setup()
    const boughtOn = await renderDialog()

    await user.clear(boughtOn)
    await user.type(boughtOn, "2999-01-01")
    await user.click(screen.getByRole("button", { name: "Save changes" }))

    expect(
      await screen.findByText("Pick today or an earlier date.")
    ).toBeVisible()
    expect(updateProperty).not.toHaveBeenCalled()
    expect(updateAnExistingIndividualTransaction).not.toHaveBeenCalled()
  })
})
