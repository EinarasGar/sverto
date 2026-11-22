import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

const getProperties = vi.fn()
const getUserAssetPairRates = vi.fn()
const searchAssets = vi.fn()
const deleteAssetPairRates = vi.fn()

const ENDPOINTS: Record<string, unknown> = {
  getProperties,
  getUserAssetPairRates,
  searchAssets,
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
  apiClient: { get: vi.fn(() => Promise.resolve({ data: {} })) },
}))

const { PropertyValuations } = await import("./property-valuations")
const { renderAccounts, stubViewport, TEST_USER_ID } =
  await import("@/features/accounts/test-harness")

const PURCHASE = Date.UTC(2024, 4, 1, 12) / 1000
const LATEST = Date.UTC(2026, 10, 18, 12) / 1000

function rates(dates: number[]) {
  getUserAssetPairRates.mockResolvedValue({
    data: {
      range: "all",
      rates: dates.map((date, index) => ({ date, rate: 12000 - index * 3000 })),
    },
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  stubViewport(1440)
  getProperties.mockResolvedValue({
    data: {
      properties: [
        {
          property_id: "p-1",
          kind: "vehicle",
          name: "Web Van",
          account_id: "van",
          asset_id: 9,
          currency_asset_id: 45,
          ownership_share: 1,
          loan_account_ids: [],
        },
      ],
    },
  })
  searchAssets.mockResolvedValue({
    data: {
      results: [
        { asset_id: 45, ticker: "GBP", name: "Pound Sterling", asset_type: 1 },
      ],
    },
  })
  deleteAssetPairRates.mockResolvedValue({ data: undefined })
  rates([PURCHASE, LATEST])
})

async function renderValuations() {
  await renderAccounts(<PropertyValuations propertyId="p-1" />)
  await screen.findByRole("heading", { name: "Valuations" })
}

function rowNames(): string[] {
  return screen
    .getAllByRole("button", { name: /^Edit the / })
    .map((row) => row.getAttribute("aria-label") ?? "")
}

describe("PropertyValuations", () => {
  it("lists every valuation newest first", async () => {
    await renderValuations()
    await waitFor(() => {
      expect(rowNames()).toEqual([
        "Edit the £9,000.00 valuation from 18 Nov 2026",
        "Edit the £12,000.00 valuation from 1 May 2024",
      ])
    })
  })

  it("opens the edit dialog with the valuation filled in", async () => {
    const user = userEvent.setup()
    await renderValuations()

    await user.click(
      await screen.findByRole("button", {
        name: "Edit the £12,000.00 valuation from 1 May 2024",
      })
    )
    const dialog = await screen.findByRole("dialog")
    expect(dialog).toHaveTextContent("Edit valuation")
    expect(within(dialog).getByLabelText("Value in GBP")).toHaveValue("12000")
    expect(within(dialog).getByLabelText("Valued on")).toHaveValue("1 May 2024")
  })

  it("deletes a valuation only after confirming, over its one-second window", async () => {
    const user = userEvent.setup()
    await renderValuations()

    await user.click(
      await screen.findByRole("button", {
        name: "Delete the valuation from 1 May 2024",
      })
    )
    const dialog = await screen.findByRole("alertdialog")
    expect(dialog).toHaveTextContent("Delete valuation?")
    expect(dialog).toHaveTextContent(
      "This removes the £12,000.00 valuation from 1 May 2024."
    )
    expect(dialog).not.toHaveTextContent("no value until")

    await user.click(within(dialog).getByRole("button", { name: "Delete" }))
    await waitFor(() => {
      expect(deleteAssetPairRates).toHaveBeenCalledWith(
        TEST_USER_ID,
        9,
        45,
        PURCHASE,
        PURCHASE + 1
      )
    })
  })

  it("warns that the last valuation leaves the property without a value", async () => {
    rates([LATEST])
    const user = userEvent.setup()
    await renderValuations()

    await user.click(
      await screen.findByRole("button", {
        name: "Delete the valuation from 18 Nov 2026",
      })
    )
    expect(await screen.findByRole("alertdialog")).toHaveTextContent(
      "The property will have no value until you add a new one."
    )
  })

  it("offers to add the first valuation when there are none", async () => {
    rates([])
    await renderValuations()
    expect(await screen.findByText("No valuations yet")).toBeVisible()
  })
})
