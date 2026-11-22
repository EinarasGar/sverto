import type { ReactNode } from "react"
import type { QueryKey } from "@tanstack/react-query"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { queryKeys } from "@/lib/query"

const stubs = vi.hoisted(() => ({
  createProperty: vi.fn(),
  postCustomAssetRates: vi.fn(),
  deleteAssetPairRates: vi.fn(),
  getAccountAssetTransactions: vi.fn(),
  updateAnExistingIndividualTransaction: vi.fn(),
  getUserAssetPairRates: vi.fn(),
}))
const getTransaction = vi.hoisted(() => vi.fn())

vi.mock("@/lib/api", () => ({
  api: () => stubs,
  apiClient: { get: getTransaction },
}))

const { buildPropertiesView } = await import("./properties")
const {
  useAddPropertyValuation,
  useCreateProperty,
  useDeletePropertyValuation,
  useEditPropertyValuation,
  useSetPropertyPurchaseDate,
} = await import("./mutations")

const USER = "00000000-0000-0000-0000-000000000000"
const keys = queryKeys.user(USER)

const DERIVED: QueryKey[] = [
  keys.portfolio.history({ defaultAssetId: 45, range: "1m" }),
  keys.portfolio.overview({ defaultAssetId: 45 }),
  keys.accounts.portfolioHistory("house", { defaultAssetId: 45, range: "1m" }),
  keys.properties.list(),
]

function seededClient(extra: QueryKey[] = []): QueryClient {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  for (const key of [...DERIVED, ...extra]) client.setQueryData(key, {})
  return client
}

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
}

function invalidated(client: QueryClient, key: QueryKey): boolean {
  return client.getQueryState(key)?.isInvalidated ?? false
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("buildPropertiesView", () => {
  it("finds only real properties by id, never an inherited member", () => {
    const view = buildPropertiesView({ properties: [] })
    expect(view.byId.get("toString")).toBeUndefined()
    expect(view.byAccountId.get("constructor")).toBeUndefined()
  })
})

describe("property mutations", () => {
  it("refreshes net worth, history and the ledger even when a create fails", async () => {
    const transactions = keys.transactions.combined({ limit: 25 })
    const client = seededClient([transactions])
    stubs.createProperty.mockRejectedValue(new Error("gateway timeout"))

    const { result } = renderHook(() => useCreateProperty(USER), {
      wrapper: wrapper(client),
    })
    result.current.mutate({
      body: {
        kind: "real_estate",
        name: "Flat 12",
        currency_asset_id: 45,
        ownership_share: 1,
        value: 328000,
        loan_account_ids: [],
      },
    })

    await waitFor(() => {
      expect(result.current.isError).toBe(true)
    })
    for (const key of [...DERIVED, transactions]) {
      expect(invalidated(client, key)).toBe(true)
    }
  })

  it("refreshes net worth and history after a new value", async () => {
    const client = seededClient()
    stubs.postCustomAssetRates.mockResolvedValue({ data: undefined })

    const { result } = renderHook(() => useAddPropertyValuation(USER), {
      wrapper: wrapper(client),
    })
    result.current.mutate({
      assetId: 9,
      currencyAssetId: 45,
      valuation: { date: 1_795_000_000, value: 420000 },
    })

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true)
    })
    for (const key of DERIVED) expect(invalidated(client, key)).toBe(true)
  })

  it("deletes a valuation through the one-second window around its time", async () => {
    const client = seededClient()
    stubs.deleteAssetPairRates.mockResolvedValue({ data: undefined })

    const { result } = renderHook(() => useDeletePropertyValuation(USER), {
      wrapper: wrapper(client),
    })
    result.current.mutate({
      assetId: 9,
      currencyAssetId: 45,
      date: 1_790_000_000,
    })

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true)
    })
    expect(stubs.deleteAssetPairRates).toHaveBeenCalledWith(
      USER,
      9,
      45,
      1_790_000_000,
      1_790_000_001
    )
    for (const key of DERIVED) expect(invalidated(client, key)).toBe(true)
  })

  it("adds the moved valuation before deleting the old one, and keeps the old one if the add fails", async () => {
    const calls: string[] = []
    stubs.postCustomAssetRates.mockImplementation(() => {
      calls.push("add")
      return Promise.reject(new Error("conflict"))
    })
    stubs.deleteAssetPairRates.mockImplementation(() => {
      calls.push("delete")
      return Promise.resolve({ data: undefined })
    })

    const { result } = renderHook(() => useEditPropertyValuation(USER), {
      wrapper: wrapper(seededClient()),
    })
    result.current.mutate({
      assetId: 9,
      currencyAssetId: 45,
      from: { date: 1_790_000_000, value: 310000 },
      to: { date: 1_780_000_000, value: 305000 },
    })

    await waitFor(() => {
      expect(result.current.isError).toBe(true)
    })
    expect(calls).toEqual(["add"])
  })
})

describe("useSetPropertyPurchaseDate", () => {
  const BOUGHT = 1_700_000_000
  const MOVED = 1_650_000_000
  const transfer = {
    type: "asset_transfer_in" as const,
    date: BOUGHT,
    fees: null,
    entry: { entry_id: 16, account_id: "car", asset_id: 9, amount: 1 },
  }

  function purchaseOn(rateDates: number[]) {
    stubs.getAccountAssetTransactions.mockResolvedValue({
      data: {
        results: [
          { ...transfer, transaction_id: "later", date: BOUGHT + 86_400 },
          { ...transfer, transaction_id: "bought" },
          {
            type: "asset_transfer_out",
            transaction_id: "sold",
            date: BOUGHT - 86_400,
            entry: { ...transfer.entry, amount: -1 },
          },
        ],
        total_results: 3,
        lookup_tables: { accounts: [], assets: [] },
      },
    })
    getTransaction.mockResolvedValue({
      data: { transaction: transfer, lookup_tables: {} },
    })
    stubs.updateAnExistingIndividualTransaction.mockResolvedValue({
      data: {},
    })
    stubs.getUserAssetPairRates.mockResolvedValue({
      data: {
        range: "all",
        rates: rateDates.map((date) => ({ date, rate: 25000 })),
      },
    })
    stubs.postCustomAssetRates.mockResolvedValue({ data: undefined })
    stubs.deleteAssetPairRates.mockResolvedValue({ data: undefined })
  }

  async function move(client: QueryClient) {
    const { result } = renderHook(() => useSetPropertyPurchaseDate(USER), {
      wrapper: wrapper(client),
    })
    result.current.mutate({
      accountId: "car",
      assetId: 9,
      currencyAssetId: 45,
      date: MOVED,
    })
    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true)
    })
  }

  it("moves the earliest transfer in and the valuation sitting on its date", async () => {
    purchaseOn([BOUGHT, BOUGHT + 500])
    const transactions = keys.transactions.combined({ limit: 25 })
    const client = seededClient([transactions])

    await move(client)

    expect(stubs.getAccountAssetTransactions).toHaveBeenCalledWith(
      USER,
      "car",
      9,
      100,
      0,
      undefined,
      expect.anything()
    )
    expect(getTransaction).toHaveBeenCalledWith(
      `/api/users/${USER}/transactions/individual/bought`,
      expect.anything()
    )
    expect(stubs.updateAnExistingIndividualTransaction).toHaveBeenCalledWith(
      USER,
      "bought",
      {
        transaction: { ...transfer, date: MOVED },
      }
    )
    expect(stubs.postCustomAssetRates).toHaveBeenCalledWith(USER, 9, 45, {
      rates: [{ date: MOVED, rate: 25000 }],
    })
    expect(stubs.deleteAssetPairRates).toHaveBeenCalledWith(
      USER,
      9,
      45,
      BOUGHT,
      BOUGHT + 1
    )
    for (const key of [...DERIVED, transactions]) {
      expect(invalidated(client, key)).toBe(true)
    }
  })

  it("moves only the transfer when no valuation sits on the purchase date", async () => {
    purchaseOn([BOUGHT + 500])

    await move(seededClient())

    expect(stubs.updateAnExistingIndividualTransaction).toHaveBeenCalled()
    expect(stubs.postCustomAssetRates).not.toHaveBeenCalled()
    expect(stubs.deleteAssetPairRates).not.toHaveBeenCalled()
  })
})
