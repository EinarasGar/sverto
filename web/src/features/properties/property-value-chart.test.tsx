import { fireEvent, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import type { RatePoint } from "@/features/assets/api"

const GBP = 45
const EUR = 46

const VALUATIONS: RatePoint[] = [
  { date: new Date(2025, 9, 17), rate: 296_000 },
  { date: new Date(2026, 8, 23), rate: 310_000 },
  { date: new Date(2026, 10, 18), rate: 328_000 },
]

const requested: string[] = []

function ratesOptions(kind: string) {
  return (...args: unknown[]) => {
    requested.push(kind)
    return {
      queryKey: ["test-rates", kind, ...args],
      queryFn: () => VALUATIONS,
    }
  }
}

vi.mock("@/features/assets/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/features/assets/api")>()),
  userAssetPairRatesQueryOptions: ratesOptions("pair"),
  userAssetConvertedRatesQueryOptions: ratesOptions("converted"),
}))

vi.mock("@/features/portfolio/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/features/portfolio/api")>()),
  useRequiredBaseAssetId: () => GBP,
}))

const { PropertyValueChart } = await import("./property-value-chart")
const { renderAccounts, stubViewport } =
  await import("@/features/accounts/test-harness")

function property(currencyAssetId: number) {
  return {
    propertyId: "p-1",
    kind: "real_estate" as const,
    name: "Flat 12",
    accountId: "house",
    assetId: 9,
    currencyAssetId,
    ownershipShare: 1,
    ownershipSharePercent: 100,
    isJoint: false,
    loanAccountIds: [],
  }
}

async function renderChart(currencyAssetId = GBP) {
  await renderAccounts(
    <PropertyValueChart
      property={property(currencyAssetId)}
      currentValue={328_000}
    />
  )
  await screen.findByRole("slider")
}

beforeEach(() => {
  requested.length = 0
  stubViewport(1440)
})

describe("PropertyValueChart", () => {
  it("draws the trend through the valuations with no period to choose", async () => {
    await renderChart()
    expect(new Set(requested)).toEqual(new Set(["pair"]))
    expect(screen.queryByRole("button", { name: "1M" })).toBeNull()

    const delta = document.querySelector('[data-slot="hero-chart-delta"]')
    expect(delta).toHaveTextContent("£32,000.00")
    expect(delta).toHaveTextContent("+10.8%")
    expect(delta).toHaveTextContent("since purchase")
  })

  it("names the valuation under the scrubber and keeps the trend value as the headline", async () => {
    await renderChart()
    const scrubber = screen.getByRole("slider")

    fireEvent.keyDown(scrubber, { key: "Home" })

    await waitFor(() => {
      expect(
        document.querySelector('[data-slot="hero-chart-marker-label"]')
      ).toHaveTextContent("Valued £296,000.00 · 17 Oct 2025")
    })
    expect(scrubber.getAttribute("aria-valuetext")).toContain(
      "Valued £296,000.00 · 17 Oct 2025"
    )
    expect(
      document.querySelector('[data-slot="hero-chart-value"]')
    ).toHaveTextContent("£296,000.00")

    fireEvent.keyDown(scrubber, { key: "ArrowRight" })
    await waitFor(() => {
      expect(
        document.querySelector('[data-slot="hero-chart-marker-label"]')
      ).toBeNull()
    })
  })

  it("reads converted valuations when the property is valued in another currency", async () => {
    await renderChart(EUR)
    expect(new Set(requested)).toEqual(new Set(["converted"]))
  })
})
