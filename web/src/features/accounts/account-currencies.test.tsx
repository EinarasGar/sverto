import { cleanup, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import type { Holding } from "@/features/portfolio/api"

import type { AccountDetail } from "./api"

vi.mock("@/features/portfolio/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/features/portfolio/api")>()),
  useRequiredBaseAssetId: () => 45,
}))

const { AccountCurrencies } = await import("./account-currencies")
const { renderAccounts, stubViewport } = await import("./test-harness")

function account(overrides: Partial<AccountDetail>): AccountDetail {
  return {
    accountId: "a1",
    name: "Lloyds Current Account",
    accountTypeId: 1,
    accountTypeName: "Current",
    accountClass: "cash",
    isLiquid: true,
    isLiability: false,
    liquidityTypeId: 1,
    liquidityTypeName: "Liquid",
    ownershipShare: 1,
    ownershipSharePercent: 100,
    isJoint: false,
    identifiers: [],
    ...overrides,
  }
}

const GBP: Holding = {
  accountId: "a1",
  assetId: 45,
  units: 4130.75,
  value: 4130.75,
  asset: { assetId: 45, ticker: "GBP", name: "British Pound", assetTypeId: 1 },
  account: null,
}

describe("AccountCurrencies", () => {
  it("lists a lone currency as cash on an investment account but not as a balance elsewhere", async () => {
    stubViewport(1440)
    await renderAccounts(
      <AccountCurrencies account={account({})} holdings={[GBP]} />
    )
    expect(screen.queryByRole("heading", { name: "Balances" })).toBeNull()
    cleanup()

    await renderAccounts(
      <AccountCurrencies
        account={account({ accountClass: "investments" })}
        holdings={[GBP]}
      />
    )
    expect(screen.getByRole("heading", { name: "Cash" })).toBeInTheDocument()
    expect(screen.getByText("£4,130.75")).toBeInTheDocument()
  })
})
