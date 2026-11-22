import { describe, expect, it } from "vitest"

import type { GetAccountsResponse } from "@/api"
import { buildAccountsView } from "@/features/accounts/api"
import { buildHoldingsView } from "@/features/portfolio/api"

import { buildPropertiesView } from "./api"
import { propertyFigures } from "./figures"
import { linkableLoans } from "./loans"

const HOUSE = "house"
const MORTGAGE = "mortgage"
const SECOND_CHARGE = "second-charge"
const FREE_LOAN = "free-loan"
const OTHER_HOUSE_LOAN = "other-house-loan"
const CURRENT = "current"

function account(account_id: string, name: string, account_type: number) {
  return {
    account_id,
    name,
    account_type,
    liquidity_type: 1,
    ownership_share: 0.5,
    suggested_currency: null,
  }
}

const accountsResponse: GetAccountsResponse = {
  accounts: [
    account(HOUSE, "Flat 12", 9),
    account(MORTGAGE, "Halifax Mortgage", 7),
    account(SECOND_CHARGE, "Second Charge", 8),
    account(FREE_LOAN, "Car Loan", 8),
    account(OTHER_HOUSE_LOAN, "Cottage Mortgage", 7),
    account(CURRENT, "Lloyds Current", 1),
  ],
  lookup_tables: {
    account_types: [
      { id: 1, name: "Current" },
      { id: 7, name: "Mortgage" },
      { id: 8, name: "Loan" },
      { id: 9, name: "Real Estate" },
    ],
    account_liquidity_types: [{ id: 1, name: "Liquid" }],
    assets: [],
  },
}

const properties = buildPropertiesView({
  properties: [
    {
      property_id: "p-1",
      kind: "real_estate",
      name: "Flat 12",
      account_id: HOUSE,
      asset_id: 9,
      currency_asset_id: 1,
      ownership_share: 0.5,
      loan_account_ids: [MORTGAGE, SECOND_CHARGE],
    },
    {
      property_id: "p-2",
      kind: "real_estate",
      name: "Cottage",
      account_id: "cottage",
      asset_id: 10,
      currency_asset_id: 1,
      ownership_share: 1,
      loan_account_ids: [OTHER_HOUSE_LOAN],
    },
  ],
})

const accounts = buildAccountsView(accountsResponse)

function wholeHoldings(houseValue: number | null) {
  return buildHoldingsView(
    {
      holdings: [
        { account_id: HOUSE, asset_id: 9, units: 1, value: houseValue },
        { account_id: MORTGAGE, asset_id: 1, units: -150000, value: -150000 },
        {
          account_id: SECOND_CHARGE,
          asset_id: 1,
          units: -10000,
          value: -10000,
        },
      ],
      lookup_tables: { accounts: [], assets: [] },
    },
    false
  )
}

describe("propertyFigures", () => {
  const property = properties.byId.get("p-1")!

  it("reports whole figures and loan-to-value across every linked loan", () => {
    const figures = propertyFigures(property, accounts, wholeHoldings(320000))
    expect(figures.value).toBe(320000)
    expect(figures.loanBalance).toBe(-160000)
    expect(figures.equity).toBe(160000)
    expect(figures.loanToValue).toBe(0.5)
    expect(
      figures.loans.map((loan) => [loan.account?.name, loan.balance])
    ).toEqual([
      ["Halifax Mortgage", -150000],
      ["Second Charge", -10000],
    ])
  })

  it("has no value, equity or loan-to-value when the property cannot be priced", () => {
    const figures = propertyFigures(property, accounts, wholeHoldings(null))
    expect(figures.value).toBeNull()
    expect(figures.equity).toBeNull()
    expect(figures.loanToValue).toBeNull()
    expect(figures.loanBalance).toBe(-160000)
  })
})

describe("linkableLoans", () => {
  it("offers only mortgages and loans no property has linked", () => {
    expect(
      linkableLoans(accounts, properties).map((loan) => loan.name)
    ).toEqual(["Car Loan"])
  })
})
