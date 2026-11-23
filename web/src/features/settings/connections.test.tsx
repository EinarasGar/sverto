import {
  cleanup,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import type { ConnectorBinding, ConnectorConnection } from "@/api"

const getMe = vi.fn()
const listConnections = vi.fn()
const listBindings = vi.fn()
const listProviderAccounts = vi.fn()
const getAccounts = vi.fn()
const getAccountTypes = vi.fn()
const getAccountLiquidityTypes = vi.fn()
const updateBinding = vi.fn()
const createBinding = vi.fn()
const deleteBinding = vi.fn()
const syncBinding = vi.fn()
const revokeConnection = vi.fn()
const createConnection = vi.fn()
const createOauthSession = vi.fn()
const createConnectorImport = vi.fn()

const ENDPOINTS: Record<string, unknown> = {
  getMe,
  listConnections,
  listBindings,
  listProviderAccounts,
  getAccounts,
  getAccountTypes,
  getAccountLiquidityTypes,
  updateBinding,
  createBinding,
  deleteBinding,
  syncBinding,
  revokeConnection,
  createConnection,
  createOauthSession,
  createConnectorImport,
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

const { SettingsScreen } = await import("./settings-screen")
const { seedAuthMe } = await import("@/auth/test-harness")
const { createTestQueryClient, renderSettings, stubViewport, VIEWPORTS } =
  await import("./test-harness")
const {
  BINDING_UNBOUND_CONSEQUENCE,
  CONNECTION_IMPORT_TOTAL_UNAVAILABLE,
  REVOKE_CONNECTION_BODY,
  TRUSTED_WRITES_OFF,
  TRUSTED_WRITES_ON,
} = await import("./copy")

const NOW = Math.floor(Date.now() / 1000)

function binding(overrides: Partial<ConnectorBinding> = {}): ConnectorBinding {
  return {
    id: "b1",
    connection_id: "c1",
    provider_account_id: "pa1",
    sverto_account_id: "a1",
    status: "active",
    write_mode: "ghost",
    created_at: NOW - 10_000,
    updated_at: NOW - 10_000,
    last_sync_at: NOW - 14 * 60,
    ...overrides,
  }
}

function connection(
  overrides: Partial<ConnectorConnection> = {}
): ConnectorConnection {
  return {
    id: "c1",
    provider_kind: "truelayer",
    credential_mode: "stored",
    links_by_import: true,
    status: "active",
    created_at: NOW - 90 * 86_400,
    updated_at: NOW,
    consent_expires_at: NOW + 90 * 86_400,
    ...overrides,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  stubViewport(VIEWPORTS.full)
  getMe.mockResolvedValue({
    data: {
      user_id: "00000000-0000-0000-0000-000000000000",
      role: "user",
      onboarding_version: 1,
      default_asset: { id: 1, ticker: "GBP", name: "British Pound" },
      user_metadata: { username: "einaras" },
    },
  })
  listConnections.mockResolvedValue({
    data: { connections: [connection()] },
  })
  listBindings.mockResolvedValue({ data: { bindings: [binding()] } })
  listProviderAccounts.mockResolvedValue({
    data: {
      accounts: [
        {
          provider_account_id: "pa1",
          display_name: "Lloyds Current Account",
          account_type: "Account",
          currency: "GBP",
        },
        {
          provider_account_id: "pa2",
          display_name: "Lloyds Platinum Card",
          account_type: "Card",
          currency: "GBP",
        },
      ],
    },
  })
  getAccounts.mockResolvedValue({
    data: {
      accounts: [
        {
          account_id: "a1",
          name: "Lloyds Current",
          account_type: 1,
          liquidity_type: 1,
          ownership_share: 1,
          suggested_currency: null,
        },
        {
          account_id: "a2",
          name: "Lloyds Saver",
          account_type: 1,
          liquidity_type: 1,
          ownership_share: 1,
          suggested_currency: null,
        },
      ],
      lookup_tables: {
        account_types: [{ id: 1, name: "Current" }],
        account_liquidity_types: [{ id: 1, name: "Liquid" }],
        assets: [],
      },
    },
  })
  getAccountTypes.mockResolvedValue({ data: { account_types: [] } })
  getAccountLiquidityTypes.mockResolvedValue({ data: { liquidity_types: [] } })
  updateBinding.mockResolvedValue({ data: binding({ write_mode: "trusted" }) })
  createBinding.mockResolvedValue({ data: { binding_id: "b9" } })
  deleteBinding.mockResolvedValue({ data: undefined })
  syncBinding.mockResolvedValue({
    data: { binding_id: "b1", status: "queued" },
  })
  revokeConnection.mockResolvedValue({ data: undefined })
  createConnection.mockResolvedValue({ data: { connection_id: "c9" } })
  createOauthSession.mockResolvedValue({
    data: { auth_url: "https://provider.example/consent", session_id: "s1" },
  })
  createConnectorImport.mockResolvedValue({
    data: { import_session_id: "s-9" },
  })
})

afterEach(cleanup)

describe("the connections index", () => {
  it("offers the two providers Sverto actually speaks, with their live counts", async () => {
    await renderSettings(<SettingsScreen section="connections" />)
    expect((await screen.findAllByText("TrueLayer")).length).toBeGreaterThan(0)
    expect(screen.getByText("Trading 212")).toBeInTheDocument()
    expect(screen.getByText("1 connection")).toBeInTheDocument()
    expect(screen.getByText("0 connections")).toBeInTheDocument()
  })

  it("starts a real consent session and sends the browser to the provider", async () => {
    const assign = vi.fn()
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { ...window.location, assign },
    })

    await renderSettings(<SettingsScreen section="connections" />)
    fireEvent.click(
      await screen.findByRole("button", { name: "Connect a bank" })
    )

    await waitFor(() => {
      expect(createConnection).toHaveBeenCalledWith(
        "00000000-0000-0000-0000-000000000000",
        { provider_kind: "truelayer", credential_mode: "stored" }
      )
    })
    await waitFor(() => {
      expect(assign).toHaveBeenCalledWith("https://provider.example/consent")
    })
  })

  it("lists each connection with its state and a way into it", async () => {
    await renderSettings(<SettingsScreen section="connections" />)
    const manage = await screen.findByRole("link", { name: "Manage" })
    expect(manage).toHaveAttribute(
      "href",
      "/settings?section=connections&connection=c1"
    )
    expect(screen.getByText(/synced 14 minutes ago/)).toBeInTheDocument()
  })
})

describe("a connection's detail", () => {
  async function renderDetail() {
    return renderSettings(
      <SettingsScreen section="connections" connectionId="c1" />
    )
  }

  it("comes back to the list from a real back link", async () => {
    await renderDetail()
    expect(
      await screen.findByRole("link", { name: "← Connections" })
    ).toHaveAttribute("href", "/settings?section=connections")
  })

  it("refuses to invent a lifetime import count", async () => {
    await renderDetail()
    expect(
      await screen.findByText(
        new RegExp(CONNECTION_IMPORT_TOTAL_UNAVAILABLE.slice(0, 40))
      )
    ).toBeVisible()
    expect(screen.queryByText(/imported in total/)).toBeNull()
  })

  it("shows every provider account, bound or not", async () => {
    await renderDetail()
    expect(
      await screen.findByText("Lloyds Current Account")
    ).toBeInTheDocument()
    expect(screen.getByText("Lloyds Platinum Card")).toBeInTheDocument()
    expect(screen.getAllByText("Lloyds Current").length).toBeGreaterThan(0)
    expect(
      screen.getByRole("combobox", { name: "Pick an account" })
    ).toBeInTheDocument()
    expect(
      screen.getByText(new RegExp(BINDING_UNBOUND_CONSEQUENCE.slice(0, 30)))
    ).toBeVisible()
  })

  it("spells out both states of trusted writes under the switch", async () => {
    await renderDetail()
    expect(await screen.findByText(TRUSTED_WRITES_OFF)).toBeVisible()

    fireEvent.click(screen.getByRole("switch", { name: "Trusted writes" }))
    await waitFor(() => {
      expect(updateBinding).toHaveBeenCalledWith(
        "b1",
        "00000000-0000-0000-0000-000000000000",
        { status: "active", write_mode: "trusted" }
      )
    })
  })

  it("shows the on-copy once the binding posts directly", async () => {
    listBindings.mockResolvedValue({
      data: { bindings: [binding({ write_mode: "trusted" })] },
    })
    await renderDetail()
    expect(await screen.findByText(TRUSTED_WRITES_ON)).toBeVisible()
  })

  it("pauses without touching the write mode", async () => {
    await renderDetail()
    fireEvent.click(await screen.findByRole("button", { name: "Pause" }))
    await waitFor(() => {
      expect(updateBinding).toHaveBeenCalledWith(
        "b1",
        "00000000-0000-0000-0000-000000000000",
        { status: "paused", write_mode: "ghost" }
      )
    })
  })

  it("links a picked account of a connection that links directly, from the chosen start", async () => {
    listConnections.mockResolvedValue({
      data: {
        connections: [
          connection({
            provider_kind: "trading212",
            links_by_import: false,
            consent_expires_at: null,
          }),
        ],
      },
    })
    const user = userEvent.setup()
    await renderDetail()
    expect(
      await screen.findByRole("button", { name: "Link 0 accounts" })
    ).toBeDisabled()
    expect(screen.queryByRole("button", { name: /^Import \d/ })).toBeNull()

    await user.click(screen.getByRole("combobox", { name: "Pick an account" }))
    await user.click(
      await screen.findByRole("option", { name: /Lloyds Saver/ })
    )
    await user.click(screen.getByRole("button", { name: "From a date" }))
    fireEvent.change(screen.getByLabelText("Import from date"), {
      target: { value: "2026-01-15" },
    })
    await user.click(screen.getByRole("button", { name: "Link 1 account" }))

    await waitFor(() => {
      expect(createBinding).toHaveBeenCalledWith(
        "00000000-0000-0000-0000-000000000000",
        "c1",
        {
          provider_account_id: "pa2",
          sverto_account_id: "a2",
          import_from: { kind: "date", date: "2026-01-15" },
        }
      )
    })
    expect(createConnectorImport).not.toHaveBeenCalled()
  })

  it("syncs a single binding on demand", async () => {
    await renderDetail()
    fireEvent.click(await screen.findByRole("button", { name: "Sync now" }))
    await waitFor(() => {
      expect(syncBinding).toHaveBeenCalledWith(
        "00000000-0000-0000-0000-000000000000",
        "b1",
        {}
      )
    })
  })
})

describe("linking the accounts of a stored connection, with AI features off", () => {
  async function renderDetail() {
    return renderSettings(
      <SettingsScreen section="connections" connectionId="c1" />,
      seedAuthMe(createTestQueryClient(), { ai_features_enabled: false })
    )
  }

  async function chooseSaver(user: ReturnType<typeof userEvent.setup>) {
    await user.click(
      await screen.findByRole("combobox", { name: "Pick an account" })
    )
    await user.click(
      await screen.findByRole("option", { name: /Lloyds Saver/ })
    )
  }

  it("picks accounts without linking them, then imports them from after the last entry", async () => {
    const user = userEvent.setup()
    const { router } = await renderDetail()

    expect(
      await screen.findByRole("button", { name: "Import 0 accounts" })
    ).toBeDisabled()
    expect(screen.queryByRole("button", { name: /^Link \d/ })).toBeNull()

    await chooseSaver(user)
    expect(createBinding).not.toHaveBeenCalled()
    await user.click(screen.getByRole("button", { name: "Import 1 account" }))

    await waitFor(() => {
      expect(createConnectorImport).toHaveBeenCalledWith(
        "00000000-0000-0000-0000-000000000000",
        "c1",
        {
          accounts: [{ provider_account_id: "pa2", sverto_account_id: "a2" }],
          import_from: { kind: "after_last_entry" },
        }
      )
    })
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/imports/s-9")
    })
    expect(createBinding).not.toHaveBeenCalled()
  })
})

describe("the revoke confirmation", () => {
  it("states what is lost then what survives, and confirms with an outlined button", async () => {
    await renderSettings(
      <SettingsScreen section="connections" connectionId="c1" />
    )
    fireEvent.click(
      await screen.findByRole("button", { name: "Revoke access" })
    )

    const dialog = await screen.findByRole("alertdialog")
    expect(
      within(dialog).getByText(new RegExp(REVOKE_CONNECTION_BODY.slice(0, 40)))
    ).toBeVisible()
    expect(within(dialog).getByText(/stay in your ledger/)).toBeVisible()

    const confirm = within(dialog).getByRole("button", {
      name: "Revoke access",
    })
    expect(confirm.className).toContain("border-negative")
    expect(confirm.className).toContain("bg-transparent")
    expect(
      within(dialog).getByRole("button", { name: "Keep it" })
    ).toBeVisible()
  })

  it("only revokes once the confirmation is answered", async () => {
    await renderSettings(
      <SettingsScreen section="connections" connectionId="c1" />
    )
    fireEvent.click(
      await screen.findByRole("button", { name: "Revoke access" })
    )
    expect(revokeConnection).not.toHaveBeenCalled()

    const dialog = await screen.findByRole("alertdialog")
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Revoke access" })
    )
    await waitFor(() => {
      expect(revokeConnection).toHaveBeenCalledWith(
        "00000000-0000-0000-0000-000000000000",
        "c1"
      )
    })
  })
})

describe("every width", () => {
  it.each(Object.entries(VIEWPORTS))(
    "keeps a connection manageable at %s",
    async (_width, viewport) => {
      stubViewport(viewport)
      await renderSettings(
        <SettingsScreen section="connections" connectionId="c1" />
      )
      expect(
        await screen.findByRole("switch", { name: "Trusted writes" })
      ).toBeVisible()
      expect(
        screen.getByRole("button", { name: "Revoke access" })
      ).toBeVisible()
      cleanup()
    }
  )
})
