import { describe, expect, it } from "vitest"

import type {
  GetAccountsResponse,
  GetHoldingsResponse,
  GetPropertiesResponse,
} from "@/api"
import { buildHoldingsView } from "@/features/portfolio/api"
import { buildPropertiesView } from "@/features/properties/api"

import { buildAccountsView } from "./accounts"
import { buildAccountBalances } from "./balances"

const CURRENT = "11111111-1111-1111-1111-111111111111"
const ISA = "33333333-3333-3333-3333-333333333333"
const MORTGAGE = "55555555-5555-5555-5555-555555555555"
const DEACTIVATED = "66666666-6666-6666-6666-666666666666"

const accountsResponse: GetAccountsResponse = {
  accounts: [
    {
      account_id: CURRENT,
      account_type: 1,
      liquidity_type: 1,
      name: "Lloyds Current",
      ownership_share: 1,
      suggested_currency: 1,
    },
    {
      account_id: ISA,
      account_type: 3,
      liquidity_type: 1,
      name: "Trading 212 ISA",
      ownership_share: 1,
      suggested_currency: 1,
    },
    {
      account_id: MORTGAGE,
      account_type: 7,
      liquidity_type: 1,
      name: "Halifax Mortgage",
      ownership_share: 1,
      suggested_currency: 1,
    },
  ],
  lookup_tables: {
    account_types: [
      { id: 1, name: "Current" },
      { id: 3, name: "Investment" },
      { id: 7, name: "Mortgage" },
    ],
    account_liquidity_types: [{ id: 1, name: "Liquid" }],
    assets: [
      { asset_id: 1, asset_type: 1, name: "Pound Sterling", ticker: "GBP" },
    ],
  },
}

const holdingsResponse: GetHoldingsResponse = {
  holdings: [
    { account_id: CURRENT, asset_id: 1, units: 4200, value: 4200 },
    { account_id: ISA, asset_id: 2, units: 10, value: 12000 },
    { account_id: ISA, asset_id: 3, units: 1, value: null },
    { account_id: MORTGAGE, asset_id: 1, units: -144722, value: -144722 },
    { account_id: DEACTIVATED, asset_id: 1, units: 300, value: 300 },
  ],
  lookup_tables: {
    accounts: [
      { account_id: CURRENT, account_type: 1, name: "Lloyds Current" },
      { account_id: ISA, account_type: 3, name: "Trading 212 ISA" },
      { account_id: MORTGAGE, account_type: 7, name: "Halifax Mortgage" },
      { account_id: DEACTIVATED, account_type: 1, name: "Revolut Current" },
    ],
    assets: [
      { asset_id: 1, asset_type: 1, name: "Pound Sterling", ticker: "GBP" },
    ],
  },
}

const noProperties = buildPropertiesView({ properties: [] })

const balances = buildAccountBalances(
  buildAccountsView(accountsResponse),
  buildHoldingsView(holdingsResponse),
  noProperties
)

describe("buildAccountBalances", () => {
  it("attaches each account's holdings total", () => {
    expect(
      balances.accounts.map((account) => [account.name, account.value])
    ).toEqual([
      ["Lloyds Current", 4200],
      ["Trading 212 ISA", 12000],
      ["Halifax Mortgage", -144722],
    ])
  })

  it("subtotals each class and keeps liabilities last", () => {
    expect(
      balances.groups.map((group) => [group.accountClass, group.subtotal])
    ).toEqual([
      ["cash", 4200],
      ["investments", 12000],
      ["property", 0],
      ["other", 0],
      ["liabilities", -144722],
    ])
  })

  it("splits assets from liabilities and reports liquid today", () => {
    expect(balances.assetsTotal).toBe(16200)
    expect(balances.liabilitiesTotal).toBe(-144722)
    expect(balances.liquidTotal).toBe(4200)
  })

  it("does not hide holdings that belong to an unlisted account", () => {
    expect(balances.total).toBe(-128522)
    expect(balances.netWorth).toBe(-128222)
    expect(balances.unmatchedValue).toBe(300)
    expect(balances.unmatchedAccountIds).toEqual([DEACTIVATED])
  })

  it("carries the rateless count so a screen can say the total is incomplete", () => {
    expect(
      balances.accounts.find((account) => account.accountId === ISA)
        ?.ratelessCount
    ).toBe(1)
    expect(balances.ratelessCount).toBe(1)
    expect(balances.isDegraded).toBe(true)
  })

  it("gives an account with no holdings a zero rather than a gap", () => {
    const withoutHoldings = buildAccountBalances(
      buildAccountsView(accountsResponse),
      buildHoldingsView({
        holdings: [],
        lookup_tables: { accounts: [], assets: [] },
      }),
      noProperties
    )
    expect(
      withoutHoldings.accounts.every((account) => account.value === 0)
    ).toBe(true)
    expect(
      withoutHoldings.accounts.every((account) => !account.hasHoldings)
    ).toBe(true)
    expect(withoutHoldings.unmatchedValue).toBe(0)
  })
})

describe("buildAccountBalances with properties", () => {
  const HOUSE = "77777777-7777-7777-7777-777777777777"
  const LOAN = "88888888-8888-8888-8888-888888888888"

  const accounts = buildAccountsView({
    ...accountsResponse,
    accounts: [
      ...accountsResponse.accounts,
      {
        account_id: HOUSE,
        account_type: 9,
        liquidity_type: 1,
        name: "Flat 12",
        ownership_share: 0.5,
        suggested_currency: null,
      },
      {
        account_id: LOAN,
        account_type: 8,
        liquidity_type: 1,
        name: "Car Loan",
        ownership_share: 1,
        suggested_currency: 1,
      },
    ],
    lookup_tables: {
      ...accountsResponse.lookup_tables,
      account_types: [
        ...accountsResponse.lookup_tables.account_types,
        { id: 8, name: "Loan" },
        { id: 9, name: "Real Estate" },
      ],
    },
  })

  const holdings = buildHoldingsView({
    ...holdingsResponse,
    holdings: [
      ...holdingsResponse.holdings,
      { account_id: HOUSE, asset_id: 9, units: 0.5, value: 164000 },
      { account_id: LOAN, asset_id: 1, units: -5000, value: -5000 },
    ],
  })

  const properties: GetPropertiesResponse = {
    properties: [
      {
        property_id: "p-1",
        kind: "real_estate",
        name: "Flat 12",
        account_id: HOUSE,
        asset_id: 9,
        currency_asset_id: 1,
        ownership_share: 0.5,
        loan_account_ids: [MORTGAGE],
      },
    ],
  }

  const plain = buildAccountBalances(accounts, holdings, noProperties)
  const folded = buildAccountBalances(
    accounts,
    holdings,
    buildPropertiesView(properties)
  )

  it("leaves a linked loan off the list and folds it into the property row", () => {
    expect(folded.accounts.map((account) => account.name)).toEqual([
      "Lloyds Current",
      "Trading 212 ISA",
      "Flat 12",
      "Car Loan",
    ])
    const house = folded.accounts.find((account) => account.accountId === HOUSE)
    expect(house?.value).toBe(164000 - 144722)
    expect(house?.property?.propertyId).toBe("p-1")
  })

  it("keeps an unlinked loan as a normal liability row", () => {
    const loan = folded.accounts.find((account) => account.accountId === LOAN)
    expect(loan?.value).toBe(-5000)
    expect(loan?.property).toBeNull()
  })

  it("moves the loan between group subtotals without changing the totals", () => {
    expect(
      folded.groups.map((group) => [group.accountClass, group.subtotal])
    ).toEqual([
      ["cash", 4200],
      ["investments", 12000],
      ["property", 164000 - 144722],
      ["other", 0],
      ["liabilities", -5000],
    ])
    expect(folded.total).toBe(plain.total)
    expect(folded.netWorth).toBe(plain.netWorth)
    expect(folded.unmatchedValue).toBe(plain.unmatchedValue)
  })

  it("does not report a hidden loan as a deactivated account", () => {
    expect(folded.unmatchedAccountIds).toEqual([DEACTIVATED])
  })

  it("keeps a loan listed when its property's own account is not in the list", () => {
    const orphaned = buildAccountBalances(
      accounts,
      holdings,
      buildPropertiesView({
        properties: [
          {
            ...properties.properties[0]!,
            account_id: DEACTIVATED,
            loan_account_ids: [LOAN],
          },
        ],
      })
    )
    expect(orphaned.accounts.map((account) => account.name)).toContain(
      "Car Loan"
    )
    expect(orphaned.total).toBe(plain.total)
  })

  it("folds only the loans the list holds, so an unlisted one is counted once", () => {
    const stale = buildAccountBalances(
      accounts,
      holdings,
      buildPropertiesView({
        properties: [
          {
            ...properties.properties[0]!,
            loan_account_ids: [MORTGAGE, DEACTIVATED],
          },
        ],
      })
    )
    const house = stale.accounts.find((account) => account.accountId === HOUSE)
    expect(house?.value).toBe(164000 - 144722)
    expect(house?.property?.loanAccountIds).toEqual([MORTGAGE])
    expect(stale.unmatchedAccountIds).toEqual([DEACTIVATED])
    expect(stale.total + stale.unmatchedValue).toBe(stale.netWorth)
  })
})
