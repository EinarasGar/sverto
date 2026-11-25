import { describe, expect, it } from "vitest"

import { buildHoldingsView, foldHoldings } from "@/features/portfolio/api"

import {
  BTC,
  CREDIT_CARD,
  CURRENT,
  foldableHoldingsFixture,
  GBP,
  holdingsFixture,
  holdingsResponse,
  HOUSE,
  HOUSE_PROPERTY,
  IBKR,
  ISA,
  LINKED_MORTGAGE,
  overviewFixture,
  propertiesFixture,
  UNPRICED,
  VWRP,
} from "./fixtures"
import { buildHoldingRows, lotSummaryOf, summariseHoldings } from "./holdings"

const holdings = holdingsFixture()
const overview = overviewFixture()
const rows = buildHoldingRows(foldHoldings(holdings), overview)
const byKey = new Map(rows.map((row) => [row.assetId, row]))

describe("buildHoldingRows", () => {
  it("keeps every asset the user holds, including the ones the overview drops", () => {
    expect(rows.map((row) => row.assetId)).toEqual([BTC, GBP, VWRP, UNPRICED])
    expect(overview.assets.map((asset) => asset.assetId)).not.toContain(GBP)
  })

  it("attaches lifetime figures only where the overview has a position", () => {
    expect(byKey.get(VWRP)?.lifetime?.totalGains).toBeCloseTo(311.4)
    expect(byKey.get(GBP)?.lifetime).toBeNull()
    expect(byKey.get(UNPRICED)?.lifetime).toBeNull()
  })

  it("marks a currency asset as cash from its asset type, not its name", () => {
    expect(byKey.get(GBP)?.isCash).toBe(true)
    expect(byKey.get(BTC)?.isCash).toBe(false)
  })

  it("shares against the total that includes cash, so the column sums to one", () => {
    const total = rows.reduce((sum, row) => sum + row.share, 0)
    expect(total).toBeCloseTo(1)
    expect(byKey.get(GBP)?.share).toBeCloseTo(4200 / holdings.totalValue)
  })

  it("lists each account holding the asset, richest first", () => {
    expect(
      byKey.get(VWRP)?.accounts.map((account) => account.accountId)
    ).toEqual([IBKR, ISA])
    expect(byKey.get(VWRP)?.accounts[0]?.position?.totalGains).toBeCloseTo(
      103.2
    )
  })

  it("carries a rateless holding with a null value rather than a zero", () => {
    const unpriced = byKey.get(UNPRICED)
    expect(unpriced?.ratelessCount).toBe(1)
    expect(unpriced?.accounts[0]?.value).toBeNull()
    expect(unpriced?.accounts[0]?.accountId).toBe(CURRENT)
  })

  it("falls back to a stated placeholder when the lookup table omitted the asset", () => {
    expect(byKey.get(UNPRICED)?.label).toBe(`Asset ${String(UNPRICED)}`)
  })

  it("names a house without units, while other assets keep theirs", () => {
    const HOUSE = 11
    const withHouse = buildHoldingsView({
      holdings: [
        ...holdingsResponse.holdings,
        { account_id: CURRENT, asset_id: HOUSE, units: 1, value: 328000 },
      ],
      lookup_tables: {
        ...holdingsResponse.lookup_tables,
        assets: [
          ...holdingsResponse.lookup_tables.assets,
          {
            asset_id: HOUSE,
            asset_type: 8,
            name: "Flat 12",
            ticker: "HOME-30D7F2E7",
          },
        ],
      },
    })
    const house = buildHoldingRows(foldHoldings(withHouse), overview).find(
      (row) => row.assetId === HOUSE
    )
    expect(house?.label).toBe("Flat 12")
    expect(house?.subLabel).toBe("1 account")
    expect(house?.units).toBeNull()
    expect(house?.accounts[0]?.units).toBeNull()
    expect(byKey.get(VWRP)?.units).toBe(38)
  })
})

describe("summariseHoldings", () => {
  const summary = summariseHoldings(foldHoldings(holdings), overview)

  it("totals value from holdings and gains from the overview", () => {
    expect(summary.holdingsTotal).toBe(holdings.totalValue)
    expect(summary.totalGains).toBe(overview.totals.totalGains)
  })

  it("keeps cash out of the priced value it is not part of", () => {
    expect(summary.cashValue).toBe(4200)
    expect(summary.pricedValue).toBe(overview.totals.marketValue)
    expect(summary.holdingsTotal).not.toBe(summary.pricedValue)
  })

  it("counts the accounts that actually hold something", () => {
    expect(summary.accountCount).toBe(3)
    expect(summary.assetCount).toBe(4)
    expect(summary.ratelessCount).toBe(1)
  })
})

describe("folding properties and liabilities", () => {
  const view = foldableHoldingsFixture()

  it("keeps debt out of the currency row and folds linked loans into the property", () => {
    const summary = summariseHoldings(
      foldHoldings(view, propertiesFixture()),
      overview
    )
    const house = summary.rows.find((row) => row.property !== null)

    expect(summary.rows.find((row) => row.assetId === GBP)?.value).toBe(4200)
    expect(house?.key).toBe(`property:${HOUSE_PROPERTY}`)
    expect(house?.value).toBe(164000 - 80153)
    expect(house?.property).toMatchObject({
      assetValue: 164000,
      loanBalance: -80153,
    })
    expect(summary.liabilities.map((row) => row.accountId)).toEqual([
      CREDIT_CARD,
    ])
    expect(summary.holdingsTotal + summary.liabilitiesTotal).toBeCloseTo(
      view.totalValue
    )
  })

  it("lists an unlinked mortgage as a liability and the house as a plain asset", () => {
    const folded = foldHoldings(view)

    expect(folded.liabilities.map((entry) => entry.accountId)).toEqual([
      LINKED_MORTGAGE,
      CREDIT_CARD,
    ])
    expect(folded.assetsById[HOUSE]?.value).toBe(164000)
    expect(folded.assetsById[GBP]?.value).toBe(4200)
  })
})

describe("lotSummaryOf", () => {
  it("reports the merged lot ledger for an asset held in two accounts", () => {
    const holding = overview.assetsById[VWRP]
    expect(holding).toBeDefined()
    const summary = lotSummaryOf(holding!)
    expect(summary.lotCount).toBe(3)
    expect(summary.realisedGains).toBeCloseTo(64)
    expect(summary.totalFees).toBeCloseTo(6)
    expect(summary.averageUnitCost).toBeCloseTo((2171.4 + 2354.4) / 48)
  })
})
