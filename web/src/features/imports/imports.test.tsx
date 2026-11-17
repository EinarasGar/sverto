import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { AxiosError, AxiosHeaders } from "axios"
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  onTestFinished,
  vi,
} from "vitest"

import type {
  GetFileResponse,
  IdentifiableImportFile,
  IdentifiableImportSessionResponse,
  IdentifiableStagedTransaction,
  ImportClassificationStatus,
  ImportItemsResponse,
  ImportSessionDetailResponse,
  ImportSessionStatus,
  ImportUnstageableGroup,
} from "@/api"

const endpoints = vi.hoisted(() => ({
  listImportSessions: vi.fn(),
  getImportSession: vi.fn(),
  createImportSession: vi.fn(),
  deleteImportSession: vi.fn(),
  attachFiles: vi.fn(),
  detachFile: vi.fn(),
  retryFile: vi.fn(),
  readFileDates: vi.fn(),
  retryClassification: vi.fn(),
  getFile: vi.fn(),
  getAccounts: vi.fn(),
  answerAsk: vi.fn(),
  listPending: vi.fn(),
  listItems: vi.fn(),
  setItemsVerb: vi.fn(),
  applyImportSession: vi.fn(),
  getAccountTypes: vi.fn(),
  getAccountLiquidityTypes: vi.fn(),
  addAccount: vi.fn(),
}))

const uploadFile = vi.hoisted(() => vi.fn())

vi.mock("@/lib/api", () => ({
  api: () =>
    new Proxy(
      {},
      {
        get: (_target, name: string) =>
          (endpoints as Record<string, unknown>)[name] ??
          (() => Promise.resolve({ data: {} })),
      }
    ),
  apiClient: { get: vi.fn(() => Promise.resolve({ data: {} })) },
}))

vi.mock("@/features/uploads/api/upload-file", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  uploadFile,
}))

const { renderImports, TEST_USER_ID } = await import("./test-harness")

const CREATED_AT = "2026-10-19T14:32:00Z"

function summary(
  id: string,
  status: ImportSessionStatus,
  accounts: string[] = []
): IdentifiableImportSessionResponse {
  return {
    id,
    status,
    accounts,
    classification: { status: "not_started" },
    created_at: CREATED_AT,
  }
}

function attached(
  fileId: string,
  overrides: Partial<IdentifiableImportFile> = {}
): IdentifiableImportFile {
  return {
    file_id: fileId,
    status: "pending",
    created_at: CREATED_AT,
    error_message: null,
    paused_until: null,
    recorded_transactions: 0,
    ...overrides,
  }
}

function detail(
  status: ImportSessionStatus,
  files: IdentifiableImportFile[]
): ImportSessionDetailResponse {
  return {
    session: {
      status,
      accounts: [],
      classification: { status: "not_started" },
      created_at: CREATED_AT,
    },
    files,
    counts: {
      add: 0,
      amend: 0,
      delete: 0,
      skip: 0,
      pending: 0,
      unstageable: 0,
      staged_available: true,
    },
    open_asks: [],
    conversation_id: "conversation-1",
  }
}

const emptyItems: ImportItemsResponse = {
  ready: false,
  items: [],
  unstageable: [],
  read_as_owed: [],
  lookup_tables: { assets: [], accounts: [], categories: [] },
}

const FILE_NAMES: Record<string, string> = {
  "f-1": "statement.csv",
  "f-2": "scan.pdf",
  "f-new": "new.csv",
}

function fileDetails(fileId: string): GetFileResponse {
  return {
    id: fileId,
    original_name: FILE_NAMES[fileId] ?? "unknown",
    mime_type: "text/csv",
    size_bytes: 2048,
    status: "ready",
    has_thumbnail: false,
    created_at: CREATED_AT,
    updated_at: CREATED_AT,
  }
}

function giveElementsHeight(height: number) {
  const original = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "offsetHeight"
  )
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get: () => height,
  })
  onTestFinished(() => {
    if (original) {
      Object.defineProperty(HTMLElement.prototype, "offsetHeight", original)
    }
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  endpoints.getAccounts.mockResolvedValue({
    data: {
      accounts: [
        {
          account_id: "acc-1",
          name: "Northwind",
          account_type: 1,
          liquidity_type: 1,
          ownership_share: 1,
          suggested_currency: null,
        },
      ],
      lookup_tables: {
        assets: [],
        account_types: [],
        account_liquidity_types: [],
      },
    },
  })
  endpoints.getFile.mockImplementation((_userId: string, fileId: string) =>
    Promise.resolve({ data: fileDetails(fileId) })
  )
  endpoints.listImportSessions.mockResolvedValue({
    data: [summary("s-1", "parsing"), summary("s-2", "completed", ["acc-1"])],
  })
  endpoints.getImportSession.mockResolvedValue({
    data: detail("parsing", [
      attached("f-1"),
      attached("f-2", {
        status: "failed",
        error_message: "The PDF is password protected.",
      }),
    ]),
  })
  endpoints.detachFile.mockResolvedValue({ data: undefined })
  endpoints.retryFile.mockResolvedValue({ data: undefined })
  endpoints.readFileDates.mockResolvedValue({ data: undefined })
  endpoints.deleteImportSession.mockResolvedValue({ data: undefined })
  endpoints.answerAsk.mockResolvedValue({ data: undefined })
  endpoints.listPending.mockResolvedValue({ data: [] })
  endpoints.listItems.mockResolvedValue({ data: emptyItems })
  endpoints.setItemsVerb.mockResolvedValue({ data: undefined })
  endpoints.applyImportSession.mockResolvedValue({ data: undefined })
})

afterEach(() => {
  vi.clearAllMocks()
})

describe("the imports list", () => {
  it("starting a new import creates a session and opens it", async () => {
    endpoints.createImportSession.mockResolvedValue({
      data: summary("s-new", "collecting"),
    })
    endpoints.getImportSession.mockResolvedValue({
      data: detail("collecting", []),
    })
    const user = userEvent.setup()
    const { router } = renderImports("/imports")

    const [headerAction] = await screen.findAllByRole("button", {
      name: "New import",
    })
    await user.click(headerAction as HTMLElement)

    expect(await screen.findByText("Add the files to import")).toBeVisible()
    expect(endpoints.createImportSession).toHaveBeenCalledWith(TEST_USER_ID)
    expect(router.state.location.pathname).toBe("/imports/s-new")
    expect(screen.getByText("Waiting for files")).toBeVisible()
  })

  it("asks to turn on AI features before starting an import", async () => {
    const user = userEvent.setup()
    renderImports("/imports", { aiFeaturesEnabled: false })

    const [headerAction] = await screen.findAllByRole("button", {
      name: "New import",
    })
    await user.click(headerAction as HTMLElement)

    expect(await screen.findByRole("alertdialog")).toBeVisible()
    expect(endpoints.createImportSession).not.toHaveBeenCalled()
  })
})

describe("an import session", () => {
  it("removing a file detaches it from the import", async () => {
    const user = userEvent.setup()
    renderImports("/imports/s-1")
    const remove = await screen.findByRole("button", {
      name: "Remove statement.csv",
    })
    endpoints.getImportSession.mockResolvedValue({
      data: detail("parsing", [attached("f-2", { status: "failed" })]),
    })

    await user.click(remove)

    expect(endpoints.detachFile).toHaveBeenCalledWith(
      TEST_USER_ID,
      "s-1",
      "f-1"
    )
    await waitFor(() => {
      expect(screen.queryByText("statement.csv")).not.toBeInTheDocument()
    })
  })

  it("a file that could not be read can be retried", async () => {
    const user = userEvent.setup()
    endpoints.getImportSession.mockResolvedValue({
      data: detail("parsing", [
        attached("f-1", {
          status: "failed",
          error_message: "This file couldn't be read after 5 attempts.",
        }),
      ]),
    })
    renderImports("/imports/s-1")
    const retry = await screen.findByRole("button", {
      name: "Retry statement.csv",
    })

    await user.click(retry)

    expect(endpoints.retryFile).toHaveBeenCalledWith(TEST_USER_ID, "s-1", "f-1")
  })

  it("reads a file whose dates read both ways in the other order", async () => {
    const user = userEvent.setup()
    endpoints.getImportSession.mockResolvedValue({
      data: detail("parsing", [
        attached("f-1", { status: "parsed", date_order: "day_first" }),
      ]),
    })
    renderImports("/imports/s-1")

    expect(
      await screen.findByText(
        /They are read day first, so 03\/11 is 3 November/
      )
    ).toBeVisible()
    await user.click(
      await screen.findByRole("button", {
        name: "Read the dates in statement.csv month first",
      })
    )

    expect(endpoints.readFileDates).toHaveBeenCalledWith(
      TEST_USER_ID,
      "s-1",
      "f-1",
      { date_order: "month_first" }
    )
  })

  it("shows a failed classification and retries it", async () => {
    const user = userEvent.setup()
    const failed = detail("needs_input", [
      attached("f-1", { status: "parsed" }),
    ])
    failed.session.classification = {
      status: "failed",
      error_message: "Your AI usage limit was reached. Retry later.",
    }
    endpoints.getImportSession.mockResolvedValue({ data: failed })
    renderImports("/imports/s-1")

    const row = await screen.findByText(
      "Your AI usage limit was reached. Retry later."
    )
    await user.click(
      within(row.parentElement as HTMLElement).getByRole("button", {
        name: "Retry categorizing",
      })
    )

    expect(endpoints.retryClassification).toHaveBeenCalledWith(
      TEST_USER_ID,
      "s-1"
    )
  })

  it("keeps refreshing only while transactions wait for or are being categorized", async () => {
    const { readingRefetchInterval } = await import("./api")
    const withClassification = (status: ImportClassificationStatus) => {
      const session = detail("reconciling", [
        attached("f-1", { status: "parsed" }),
      ])
      session.session.classification = { status }
      return readingRefetchInterval(session)
    }

    expect(withClassification("waiting")).toBe(3000)
    expect(withClassification("running")).toBe(3000)
    expect(withClassification("classified")).toBe(false)
    expect(withClassification("failed")).toBe(false)
  })

  it("keeps refreshing only while a file is being read", async () => {
    const { readingRefetchInterval } = await import("./api")

    expect(
      readingRefetchInterval(
        detail("parsing", [attached("f-1", { status: "processing" })])
      )
    ).toBe(3000)
    expect(
      readingRefetchInterval(
        detail("reconciling", [
          attached("f-1", { status: "parsed" }),
          attached("f-2", { status: "failed" }),
        ])
      )
    ).toBe(false)
  })

  it("attaches an uploaded file to the import", async () => {
    uploadFile.mockResolvedValue("f-new")
    endpoints.attachFiles.mockResolvedValue({ data: [attached("f-new")] })
    const user = userEvent.setup()
    renderImports("/imports/s-1")

    await screen.findByText("statement.csv")
    endpoints.getImportSession.mockResolvedValue({
      data: detail("parsing", [attached("f-1"), attached("f-new")]),
    })
    await user.upload(
      screen.getByLabelText("Choose files"),
      new File(["date,amount"], "new.csv", { type: "text/csv" })
    )

    expect(await screen.findByText("new.csv")).toBeVisible()
    expect(endpoints.attachFiles).toHaveBeenCalledWith(TEST_USER_ID, "s-1", {
      file_ids: ["f-new"],
    })
  })

  it("uploads a statement the browser cannot name a type for", async () => {
    uploadFile.mockResolvedValue("f-new")
    endpoints.attachFiles.mockResolvedValue({ data: [attached("f-new")] })
    const user = userEvent.setup()
    renderImports("/imports/s-1")

    await screen.findByText("statement.csv")
    const statement = new File(["OFXHEADER:100"], "statement.ofx", { type: "" })
    await user.upload(screen.getByLabelText("Choose files"), statement)

    await waitFor(() => {
      expect(uploadFile).toHaveBeenCalledWith(
        expect.objectContaining({ file: statement })
      )
    })
    expect(screen.queryByText(/Sverto reads/)).not.toBeInTheDocument()
  })

  it("deleting asks first, then returns to the list", async () => {
    const user = userEvent.setup()
    const { router } = renderImports("/imports/s-1")

    await user.click(
      await screen.findByRole("button", { name: "Delete import" })
    )
    const dialog = await screen.findByRole("alertdialog")
    expect(endpoints.deleteImportSession).not.toHaveBeenCalled()
    await user.click(
      within(dialog).getByRole("button", { name: "Delete import" })
    )

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/imports")
    })
    expect(endpoints.deleteImportSession).toHaveBeenCalledWith(
      TEST_USER_ID,
      "s-1"
    )
  })

  function accountAsk(
    fileId: string,
    name: string,
    candidates: { id: string; name: string }[] = [],
    answerCoversAllFiles = false
  ) {
    return {
      kind: "account_mapping" as const,
      ask_key: `account_mapping:${fileId}:${name}`,
      file_id: fileId,
      raw_account: name,
      candidates,
      choices: ["map" as const, "exclude" as const],
      answer_covers_all_files: answerCoversAllFiles,
    }
  }

  function notOpen() {
    const config = { headers: new AxiosHeaders() }
    return new AxiosError(
      "Request failed",
      "ERR_BAD_REQUEST",
      config,
      {},
      {
        status: 404,
        statusText: "",
        data: {
          error_type: "NotFound",
          message: "Ask is not open or does not exist.",
          errors: [],
        },
        headers: new AxiosHeaders(),
        config,
      }
    )
  }

  it("one answer links every file that names the same account", async () => {
    const user = userEvent.setup()
    endpoints.getImportSession.mockResolvedValue({
      data: {
        ...detail("needs_input", [
          attached("f-1", { status: "parsed" }),
          attached("f-2", { status: "parsed" }),
        ]),
        open_asks: [
          accountAsk("f-1", "Current", [{ id: "acc-1", name: "Northwind" }]),
          accountAsk("f-2", "Current", [{ id: "acc-1", name: "Northwind" }]),
        ],
      },
    })
    renderImports("/imports/s-1")

    expect(await screen.findByText("Which account is “Current”?")).toBeVisible()
    await user.click(screen.getByRole("button", { name: "Link" }))

    await waitFor(() => {
      expect(endpoints.answerAsk).toHaveBeenCalledTimes(2)
    })
    for (const fileId of ["f-1", "f-2"]) {
      expect(endpoints.answerAsk).toHaveBeenCalledWith(
        TEST_USER_ID,
        "s-1",
        `account_mapping:${fileId}:Current`,
        { kind: "account_mapping", choice: "map", account_id: "acc-1" }
      )
    }
  })

  it("a question settled by an earlier answer in the group is not an error", async () => {
    const user = userEvent.setup()
    endpoints.getImportSession.mockResolvedValue({
      data: {
        ...detail("needs_input", [
          attached("f-1", { status: "parsed" }),
          attached("f-2", { status: "parsed" }),
        ]),
        open_asks: [
          accountAsk("f-1", "31926819", [{ id: "acc-1", name: "Northwind" }]),
          accountAsk("f-2", "31926819", [{ id: "acc-1", name: "Northwind" }]),
        ],
      },
    })
    endpoints.answerAsk
      .mockResolvedValueOnce({ data: undefined })
      .mockRejectedValueOnce(notOpen())
    renderImports("/imports/s-1")

    await user.click(await screen.findByRole("button", { name: "Link" }))

    await waitFor(() => {
      expect(endpoints.answerAsk).toHaveBeenCalledTimes(2)
    })
    expect(
      screen.queryByText("The answer could not be saved")
    ).not.toBeInTheDocument()
  })

  it("an account can be left out of the import", async () => {
    const user = userEvent.setup()
    endpoints.getImportSession.mockResolvedValue({
      data: {
        ...detail("needs_input", [attached("f-1", { status: "parsed" })]),
        open_asks: [accountAsk("f-1", "Pocket")],
      },
    })
    renderImports("/imports/s-1")

    await user.click(
      await screen.findByRole("button", { name: "Don’t import" })
    )

    await waitFor(() => {
      expect(endpoints.answerAsk).toHaveBeenCalledWith(
        TEST_USER_ID,
        "s-1",
        "account_mapping:f-1:Pocket",
        { kind: "account_mapping", choice: "exclude" }
      )
    })
  })

  function assetAsk(raw: string, choices: ("map" | "exclude")[]) {
    return {
      kind: "asset_mapping" as const,
      ask_key: `asset_mapping:${raw}`,
      raw,
      candidates: [],
      choices,
    }
  }

  it("an asset is left out only after its consequence is shown", async () => {
    const user = userEvent.setup()
    endpoints.getImportSession.mockResolvedValue({
      data: {
        ...detail("needs_input", [attached("f-1", { status: "parsed" })]),
        open_asks: [assetAsk("SPX500", ["map", "exclude"])],
        counts: {
          add: 0,
          amend: 0,
          delete: 0,
          skip: 0,
          pending: 3,
          unstageable: 0,
          staged_available: true,
        },
      },
    })
    const waiting = (id: string, legs: [string, string][]) => ({
      id,
      file_id: "f-1",
      account_ref: "Broker",
      date: "2026-02-24T14:24:00Z",
      description: id,
      legs: legs.map(([asset_ref, quantity]) => ({ asset_ref, quantity })),
      blocked_by: ["asset_mapping:SPX500"],
    })
    endpoints.listPending.mockResolvedValue({
      data: [
        waiting("l-1", [
          ["SPX500", "1"],
          ["USD", "-550"],
        ]),
        waiting("l-2", [["SPX500", "2"]]),
        waiting("l-3", [["USD", "-3"]]),
      ],
    })
    renderImports("/imports/s-1")

    expect(await screen.findByText("Which asset is “SPX500”?")).toBeVisible()
    expect(screen.getByRole("button", { name: "Link" })).toBeDisabled()
    await user.click(
      screen.getByRole("button", { name: "Leave this asset out" })
    )

    expect(endpoints.answerAsk).not.toHaveBeenCalled()
    expect(
      await screen.findByText(
        "2 rows of this import move “SPX500”. 1 row moves nothing else and will not be imported. On the other 1 row, what was paid for it or received from it is recorded as a transfer. Balances stated for it are not checked, and this answer can’t be changed afterwards."
      )
    ).toBeVisible()
    await user.click(screen.getByRole("button", { name: "Leave it out" }))

    await waitFor(() => {
      expect(endpoints.answerAsk).toHaveBeenCalledWith(
        TEST_USER_ID,
        "s-1",
        "asset_mapping:SPX500",
        { kind: "asset_mapping", choice: "exclude" }
      )
    })
  })

  it("a new account starts from the name on the statement and is linked once saved", async () => {
    const user = userEvent.setup()
    endpoints.getImportSession.mockResolvedValue({
      data: {
        ...detail("needs_input", [attached("f-1", { status: "parsed" })]),
        open_asks: [accountAsk("f-1", "LT12 1000 0111 0100 1000")],
      },
    })
    endpoints.getAccountTypes.mockResolvedValue({
      data: { account_types: [{ id: 1, name: "Current" }] },
    })
    endpoints.getAccountLiquidityTypes.mockResolvedValue({
      data: { account_liquidity_types: [{ id: 1, name: "Liquid" }] },
    })
    renderImports("/imports/s-1")

    await user.click(
      await screen.findByRole("button", { name: "Create account" })
    )

    expect(
      await screen.findByDisplayValue("LT12 1000 0111 0100 1000")
    ).toBeVisible()
    expect(screen.getByDisplayValue("LT121000011101001000")).toBeVisible()

    endpoints.addAccount.mockResolvedValue({ data: { account_id: "acc-new" } })
    await user.click(screen.getByRole("button", { name: "Add account" }))

    await waitFor(() => {
      expect(endpoints.answerAsk).toHaveBeenCalledWith(
        TEST_USER_ID,
        "s-1",
        "account_mapping:f-1:LT12 1000 0111 0100 1000",
        { kind: "account_mapping", choice: "map", account_id: "acc-new" }
      )
    })
    expect(endpoints.addAccount).toHaveBeenCalledWith(
      TEST_USER_ID,
      expect.objectContaining({
        name: "LT12 1000 0111 0100 1000",
        identifiers: [{ kind: "iban", value: "LT121000011101001000" }],
      })
    )
  })

  it("refetches the waiting rows when the import changes underneath them", async () => {
    giveElementsHeight(480)
    const user = userEvent.setup()
    const waiting = (fileId: string, description: string) => ({
      id: `l-${fileId}`,
      file_id: fileId,
      account_ref: "Current",
      date: "2026-02-24T14:24:00Z",
      description,
      legs: [{ asset_ref: "GBP", quantity: "-550.00" }],
      blocked_by: [`account_mapping:${fileId}:Current`],
    })
    const withPending = (files: IdentifiableImportFile[], pending: number) => ({
      ...detail("needs_input", files),
      open_asks: files.map((file) => accountAsk(file.file_id, "Current")),
      counts: { ...detail("needs_input", files).counts, pending },
    })
    endpoints.getImportSession.mockResolvedValue({
      data: withPending(
        [
          attached("f-1", { status: "parsed" }),
          attached("f-2", { status: "parsed" }),
        ],
        2
      ),
    })
    endpoints.listPending.mockResolvedValue({
      data: [waiting("f-1", "SWIFT Transfer"), waiting("f-2", "Card payment")],
    })
    renderImports("/imports/s-1")
    expect(await screen.findByText("SWIFT Transfer")).toBeVisible()

    endpoints.getImportSession.mockResolvedValue({
      data: withPending([attached("f-2", { status: "parsed" })], 1),
    })
    endpoints.listPending.mockResolvedValue({
      data: [waiting("f-2", "Card payment")],
    })
    await user.click(
      await screen.findByRole("button", { name: "Remove statement.csv" })
    )

    await waitFor(() => {
      expect(screen.queryByText("SWIFT Transfer")).not.toBeInTheDocument()
    })
    expect(screen.getByText("Card payment")).toBeVisible()
  })

  function classifiedDetail(): ImportSessionDetailResponse {
    const classified = detail("reconciling", [
      attached("f-1", { status: "parsed" }),
    ])
    classified.session.classification = { status: "classified" }
    return classified
  }

  function tescoRow(
    overrides: Partial<IdentifiableStagedTransaction> = {}
  ): IdentifiableStagedTransaction {
    return {
      id: "row-1",
      kind: "movement",
      file_ids: ["f-1"],
      verb: "add",
      verb_locked: false,
      near_miss_candidates: [],
      parts: [],
      transaction: {
        type: "regular",
        transaction_id: null,
        date: 1769940000,
        description: "Tesco",
        category_id: 76,
        entry: {
          entry_id: null,
          account_id: "acc-1",
          asset_id: 1,
          amount: -12,
        },
      },
      ...overrides,
    }
  }

  function itemsPage(
    items: IdentifiableStagedTransaction[],
    unstageable: ImportUnstageableGroup[] = []
  ): ImportItemsResponse {
    return {
      ...emptyItems,
      ready: true,
      items,
      unstageable,
    }
  }

  it("lists what the import would write and lets a row be skipped", async () => {
    giveElementsHeight(480)
    const classified = classifiedDetail()
    classified.counts = { ...classified.counts, add: 1, unstageable: 1 }
    endpoints.getImportSession.mockResolvedValue({ data: classified })
    endpoints.listItems.mockResolvedValue({
      data: itemsPage(
        [tescoRow()],
        [
          {
            id: "u-1",
            file_ids: ["f-1"],
            date: "2026-02-02T00:00:00Z",
            description: "Tax",
            reason: "Withholding tax without a dividend.",
          },
        ]
      ),
    })
    renderImports("/imports/s-1")

    expect(await screen.findByText("Tesco")).toBeVisible()
    expect(
      screen.getByText("Withholding tax without a dividend.")
    ).toBeVisible()
    await userEvent.click(
      screen.getByRole("button", { name: /^Skip Tesco on 1 Feb/ })
    )
    expect(endpoints.setItemsVerb).toHaveBeenCalledWith(TEST_USER_ID, "s-1", {
      item_ids: ["row-1"],
      verb: "skip",
    })
  })

  it("applying sends the proposal that was reviewed", async () => {
    giveElementsHeight(480)
    const ready = classifiedDetail()
    ready.session.status = "proposal_ready"
    endpoints.getImportSession.mockResolvedValue({ data: ready })
    endpoints.listItems.mockResolvedValue({
      data: {
        ...itemsPage([tescoRow(), tescoRow({ id: "row-2", verb: "skip" })]),
        fingerprint: "fp-1",
      },
    })
    renderImports("/imports/s-1")

    expect(
      await screen.findByText(
        "Applying adds 1 transaction; 1 row already in your ledger."
      )
    ).toBeVisible()
    await userEvent.click(
      screen.getByRole("button", { name: "Apply to ledger" })
    )
    await userEvent.click(await screen.findByRole("button", { name: "Apply" }))

    expect(endpoints.applyImportSession).toHaveBeenCalledWith(
      TEST_USER_ID,
      "s-1",
      { fingerprint: "fp-1" }
    )
  })

  it("starting balances wait for a decision, one by one or all at once", async () => {
    giveElementsHeight(480)
    const ready = classifiedDetail()
    ready.session.status = "proposal_ready"
    endpoints.getImportSession.mockResolvedValue({ data: ready })
    endpoints.listItems.mockResolvedValue({
      data: {
        ...itemsPage([
          tescoRow({ id: "start-1", kind: "starting_balance" }),
          tescoRow({ id: "start-2", kind: "starting_balance" }),
          tescoRow({ id: "row-3" }),
        ]),
        fingerprint: "fp-1",
      },
    })
    renderImports("/imports/s-1")

    expect(
      await screen.findByText("Decide the starting balances first.")
    ).toBeVisible()
    expect(
      screen.getByRole("button", { name: "Apply to ledger" })
    ).toBeDisabled()
    const starts = within(
      screen.getByRole("list", { name: "Starting balances" })
    )
    expect(starts.getAllByRole("listitem")).toHaveLength(2)

    await userEvent.click(screen.getByRole("button", { name: "Add all" }))
    expect(endpoints.setItemsVerb).toHaveBeenCalledWith(TEST_USER_ID, "s-1", {
      item_ids: ["start-1", "start-2"],
      verb: "add",
    })

    const [leaveOutFirst] = starts.getAllByRole("button", {
      name: /^Leave out the starting balance/,
    })
    if (leaveOutFirst === undefined) throw new Error("no Leave out button")
    await userEvent.click(leaveOutFirst)
    expect(endpoints.setItemsVerb).toHaveBeenLastCalledWith(
      TEST_USER_ID,
      "s-1",
      { item_ids: ["start-1"], verb: "skip" }
    )
  })

  it("a transfer joined from two rows can be kept apart", async () => {
    giveElementsHeight(480)
    const ready = classifiedDetail()
    ready.session.status = "proposal_ready"
    endpoints.getImportSession.mockResolvedValue({ data: ready })
    endpoints.listItems.mockResolvedValue({
      data: {
        ...itemsPage([
          tescoRow({ id: "joined-1", kind: "joined_transfer" }),
          tescoRow({ id: "row-2" }),
        ]),
        fingerprint: "fp-1",
      },
    })
    renderImports("/imports/s-1")

    const [keepApart, ...others] = await screen.findAllByRole("button", {
      name: /^Keep apart /,
    })
    if (keepApart === undefined) throw new Error("no Keep apart button")
    expect(others).toHaveLength(0)
    expect(screen.getByText(/^Moved between your accounts/)).toBeVisible()

    await userEvent.click(keepApart)
    expect(endpoints.setItemsVerb).toHaveBeenLastCalledWith(
      TEST_USER_ID,
      "s-1",
      { item_ids: ["joined-1"], verb: "apart" }
    )
  })

  it("a possible duplicate links the chosen transaction or stays new", async () => {
    giveElementsHeight(480)
    endpoints.getImportSession.mockResolvedValue({ data: classifiedDetail() })
    endpoints.listItems.mockResolvedValue({
      data: itemsPage([
        tescoRow({
          near_miss_candidates: [
            {
              id: "tx-1",
              in_ledger: true,
              confirmed: true,
              amend: false,
              leaves_movement_unrecorded: false,
              parts: [],
              date: "2026-01-31T00:00:00Z",
              description: "Tesco stores",
              account_id: null,
              asset_id: 1,
              quantity: "-5",
            },
          ],
        }),
      ]),
    })
    renderImports("/imports/s-1")

    expect(await screen.findByText("Is this already recorded?")).toBeVisible()
    await userEvent.click(screen.getByRole("button", { name: /^Same as / }))
    expect(endpoints.setItemsVerb).toHaveBeenCalledWith(TEST_USER_ID, "s-1", {
      item_ids: ["row-1"],
      verb: "skip",
      candidate_id: "tx-1",
    })

    await userEvent.click(
      screen.getByRole("button", { name: "It's a new transaction" })
    )
    expect(endpoints.setItemsVerb).toHaveBeenLastCalledWith(
      TEST_USER_ID,
      "s-1",
      { item_ids: ["row-1"], verb: "add" }
    )
  })

  it("shows each account as balanced or names the period that is off", async () => {
    giveElementsHeight(480)
    endpoints.getImportSession.mockResolvedValue({ data: classifiedDetail() })
    const page = itemsPage([tescoRow()])
    endpoints.listItems.mockResolvedValue({
      data: {
        ...page,
        balance_checks: {
          unchecked: 2,
          entered: [],
          checks: [
            {
              account_id: "acc-1",
              asset_id: 1,
              first_date: "2026-01-01T00:00:00Z",
              last_date: "2026-01-31T00:00:00Z",
              balances_checked: 2,
              starting_off_by: "0",
              final_off_by: "0",
              changes: [],
              unlisted: [],
            },
            {
              account_id: "acc-2",
              asset_id: 1,
              first_date: "2026-01-01T00:00:00Z",
              last_date: "2026-01-31T00:00:00Z",
              balances_checked: 5,
              starting_off_by: "0",
              final_off_by: "-60",
              changes: [
                {
                  after: "2026-01-07T00:00:00Z",
                  through: "2026-01-14T00:00:00Z",
                  change: "-60",
                  off_by: "-60",
                  explained: false,
                  awaits_answer: false,
                },
              ],
              unlisted: [],
            },
          ],
        },
      },
    })
    renderImports("/imports/s-1")

    expect(await screen.findByText("Balanced")).toBeVisible()
    expect(screen.getByText(/^Off by -60/)).toBeVisible()
    expect(screen.getByText(/7 Jan to 14 Jan: -60/)).toBeVisible()
    expect(
      screen.getByText(
        "2 account balances couldn't be checked: the statements give nothing to compare with."
      )
    ).toBeVisible()
  })

  const savingsLookup = {
    accounts: [{ account_id: "acc-1", name: "Savings", account_type: 1 }],
    assets: [{ asset_id: 1, ticker: "GBP", name: "Pound", asset_type: 1 }],
    categories: [],
  }

  it("asks for a balance no statement states and checks the number entered", async () => {
    giveElementsHeight(480)
    const detail = classifiedDetail()
    detail.open_asks = [
      {
        kind: "no_checkpoint",
        ask_key: "no_checkpoint:acc-1:1",
        account_id: "acc-1",
        asset_id: 1,
        as_of_date: "2026-01-31T00:00:00Z",
        choices: ["enter_balance"],
      },
    ]
    endpoints.getImportSession.mockResolvedValue({ data: detail })
    endpoints.listItems.mockResolvedValue({
      data: {
        ...itemsPage([tescoRow()]),
        lookup_tables: savingsLookup,
        balance_checks: { unchecked: 1, checks: [], entered: [] },
      },
    })
    renderImports("/imports/s-1")

    const field = await screen.findByRole("textbox", {
      name: "Balance of Savings in GBP at the end of 31 Jan",
    })
    expect(screen.queryByText(/couldn't be checked/)).toBeNull()
    expect(
      screen.queryByRole("list", { name: "Questions about this import" })
    ).toBeNull()

    await userEvent.type(field, "1 234.56")
    await userEvent.click(screen.getByRole("button", { name: "Check" }))
    expect(endpoints.answerAsk).toHaveBeenCalledWith(
      TEST_USER_ID,
      "s-1",
      "no_checkpoint:acc-1:1",
      { kind: "no_checkpoint", choice: "enter_balance", quantity: "1234.56" }
    )
  })

  it("marks a ledger row the statement does not list for deletion and keeps it again", async () => {
    giveElementsHeight(480)
    endpoints.getImportSession.mockResolvedValue({ data: classifiedDetail() })
    const page = (deleting: boolean) => ({
      ...itemsPage([tescoRow()]),
      balance_checks: {
        unchecked: 0,
        entered: [],
        checks: [
          {
            account_id: "acc-1",
            asset_id: 1,
            first_date: "2026-01-01T00:00:00Z",
            last_date: "2026-01-31T00:00:00Z",
            balances_checked: 2,
            starting_off_by: "0",
            final_off_by: deleting ? "0" : "-60",
            changes: deleting
              ? []
              : [
                  {
                    after: "2026-01-01T00:00:00Z",
                    through: "2026-01-31T00:00:00Z",
                    change: "-60",
                    off_by: "-60",
                    explained: true,
                    awaits_answer: false,
                  },
                ],
            unlisted: [
              {
                transaction_id: "tx-typed",
                date: "2026-01-10T00:00:00Z",
                quantity: "-60",
                description: "groceries",
                may_book_later: false,
                explains_difference: !deleting,
                ask_key: "not_on_statement:tx-typed",
                deleting,
                other_account_ids: [],
              },
            ],
          },
        ],
      },
    })
    endpoints.listItems.mockResolvedValue({ data: page(false) })
    endpoints.answerAsk.mockImplementation(() => {
      endpoints.listItems.mockResolvedValue({ data: page(true) })
      return Promise.resolve({ data: undefined })
    })
    renderImports("/imports/s-1")

    await userEvent.click(
      await screen.findByRole("button", {
        name: "Delete 10 Jan groceries when the import is applied",
      })
    )
    expect(endpoints.answerAsk).toHaveBeenCalledWith(
      TEST_USER_ID,
      "s-1",
      "not_on_statement:tx-typed",
      { kind: "not_on_statement", choice: "delete" }
    )
    expect(await screen.findByText("Deleted when you apply")).toBeVisible()
    await userEvent.click(
      screen.getByRole("button", { name: "Keep 10 Jan groceries" })
    )
    expect(endpoints.answerAsk).toHaveBeenLastCalledWith(
      TEST_USER_ID,
      "s-1",
      "not_on_statement:tx-typed",
      { kind: "not_on_statement", choice: "keep" }
    )
  })

  it("offers an adjustment for a difference nothing explains and names statements that disagree", async () => {
    giveElementsHeight(480)
    endpoints.getImportSession.mockResolvedValue({ data: classifiedDetail() })
    const adjustment = tescoRow({
      id: "adjust-1",
      kind: "adjustment",
      verb: "skip",
      transaction: {
        type: "cash_transfer_in",
        transaction_id: null,
        date: 1769817600,
        entry: { entry_id: null, account_id: "acc-1", asset_id: 1, amount: 60 },
      },
    })
    endpoints.listItems.mockResolvedValue({
      data: {
        ...itemsPage([tescoRow(), adjustment]),
        balance_checks: {
          unchecked: 0,
          entered: [],
          checks: [
            {
              account_id: "acc-1",
              asset_id: 1,
              first_date: "2026-01-01T00:00:00Z",
              last_date: "2026-01-31T00:00:00Z",
              balances_checked: 3,
              starting_off_by: "0",
              final_off_by: "-55",
              changes: [
                {
                  after: "2026-01-01T00:00:00Z",
                  through: "2026-01-31T00:00:00Z",
                  change: "-60",
                  off_by: "-60",
                  explained: false,
                  awaits_answer: false,
                },
                {
                  after: "2026-01-31T00:00:00Z",
                  through: "2026-01-31T00:00:00Z",
                  change: "5",
                  off_by: "-55",
                  explained: false,
                  awaits_answer: false,
                },
              ],
              unlisted: [],
            },
          ],
        },
      },
    })
    renderImports("/imports/s-1")

    expect(
      await screen.findByText("31 Jan: two statements disagree by 5")
    ).toBeVisible()
    expect(
      screen.getAllByRole("button", { name: /^Add an adjustment of / })
    ).toHaveLength(1)
    await userEvent.click(
      screen.getByRole("button", { name: "Add an adjustment of 60 on 31 Jan" })
    )
    expect(endpoints.setItemsVerb).toHaveBeenLastCalledWith(
      TEST_USER_ID,
      "s-1",
      { item_ids: ["adjust-1"], verb: "add" }
    )
  })

  it("asks for the open possible duplicate first where it may settle a difference", async () => {
    giveElementsHeight(480)
    endpoints.getImportSession.mockResolvedValue({ data: classifiedDetail() })
    endpoints.listItems.mockResolvedValue({
      data: {
        ...itemsPage([tescoRow()]),
        balance_checks: {
          unchecked: 0,
          entered: [],
          checks: [
            {
              account_id: "acc-1",
              asset_id: 1,
              first_date: "2026-01-01T00:00:00Z",
              last_date: "2026-01-31T00:00:00Z",
              balances_checked: 2,
              starting_off_by: "0",
              final_off_by: "-18",
              changes: [
                {
                  after: "2026-01-01T00:00:00Z",
                  through: "2026-01-31T00:00:00Z",
                  change: "-18",
                  off_by: "-18",
                  explained: false,
                  awaits_answer: true,
                },
              ],
              unlisted: [],
            },
          ],
        },
      },
    })
    renderImports("/imports/s-1")

    expect(
      await screen.findByText(
        "A possible duplicate on these dates is still open. Answer it first; it may settle this."
      )
    ).toBeVisible()
    expect(
      screen.queryByRole("button", { name: /^Add an adjustment of / })
    ).toBeNull()
  })

  it("a pinned row can only be pinned the other way or released", async () => {
    giveElementsHeight(480)
    endpoints.getImportSession.mockResolvedValue({ data: classifiedDetail() })
    endpoints.listItems.mockResolvedValue({
      data: itemsPage([tescoRow({ verb: "skip", verb_locked: true })]),
    })
    renderImports("/imports/s-1")

    expect(await screen.findByRole("img", { name: "Pinned" })).toBeVisible()
    expect(
      screen.queryByRole("button", { name: /^Skip Tesco/ })
    ).not.toBeInTheDocument()
    expect(screen.getByRole("button", { name: /^Add Tesco/ })).toBeVisible()
    await userEvent.click(
      screen.getByRole("button", { name: /^Undo pin on Tesco on 1 Feb/ })
    )
    expect(endpoints.setItemsVerb).toHaveBeenCalledWith(TEST_USER_ID, "s-1", {
      item_ids: ["row-1"],
      verb: "auto",
    })
  })
})
