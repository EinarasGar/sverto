import type {
  IdentifiableStagedTransaction,
  ImportClassificationStatus,
  ImportFileSource,
  ImportItemKind,
  ImportItemVerb,
  ImportSessionStatus,
} from "@/api"
import { formatTimeStamp } from "@/lib/format"

export type ImportTone =
  "ghost" | "brand" | "attention" | "positive" | "negative"

interface SessionStatusCopy {
  readonly label: string
  readonly tone: ImportTone
  readonly summary: string
}

export const SESSION_STATUS_COPY: Record<
  ImportSessionStatus,
  SessionStatusCopy
> = {
  collecting: {
    label: "Waiting for files",
    tone: "ghost",
    summary:
      "Add statements, exports, spreadsheets or screenshots. Sverto works out which accounts they belong to.",
  },
  parsing: {
    label: "Reading files",
    tone: "brand",
    summary:
      "Sverto is reading the files. It keeps going in the background, so it is safe to leave this page.",
  },
  needs_input: {
    label: "Needs your input",
    tone: "attention",
    summary:
      "Something needs a decision from you before the import can continue.",
  },
  reconciling: {
    label: "Reconciling",
    tone: "brand",
    summary: "Comparing what the files contain with your ledger.",
  },
  proposal_ready: {
    label: "Ready to review",
    tone: "attention",
    summary:
      "The proposed ledger changes are ready. Nothing is written until you approve them.",
  },
  completed: {
    label: "Completed",
    tone: "positive",
    summary: "The approved changes are in your ledger.",
  },
}

export const TONE_CLASSES: Record<ImportTone, string> = {
  ghost: "border-border-strong text-ink-3",
  brand: "border-brand/30 bg-brand-dim text-brand",
  attention: "border-attention/30 bg-attention-dim text-attention",
  positive: "border-positive/30 bg-positive-dim text-positive",
  negative: "border-negative/30 bg-negative-dim text-negative",
}

const MUTABLE_STATUSES = new Set<ImportSessionStatus>([
  "collecting",
  "parsing",
  "needs_input",
  "reconciling",
  "proposal_ready",
])

export function acceptsFiles(status: ImportSessionStatus): boolean {
  return MUTABLE_STATUSES.has(status)
}

const APPLY_BLOCKERS: Partial<Record<ImportSessionStatus, string>> = {
  collecting: "Add files first.",
  parsing: "Wait until every file has been read.",
  needs_input:
    "Answer the open questions and retry or remove the files that couldn't be read.",
  reconciling: "Wait until the transactions are categorized.",
}

export function applyBlocker(status: ImportSessionStatus): string | undefined {
  return APPLY_BLOCKERS[status]
}

export function isFinished(status: ImportSessionStatus): boolean {
  return status === "completed"
}

export function fileStatusLabel(file: ImportFileSource): string {
  if (file.paused_until !== null && file.paused_until !== undefined) {
    return `Paused until ${formatTimeStamp(file.paused_until)}`
  }
  switch (file.status) {
    case "pending":
      return "Waiting to be read"
    case "processing":
      return "Reading"
    case "parsed":
      return "Read"
    case "failed":
      return "Could not be read"
  }
}

export function fileStatusTone(file: ImportFileSource): ImportTone {
  if (file.paused_until !== null && file.paused_until !== undefined) {
    return "attention"
  }
  switch (file.status) {
    case "pending":
      return "ghost"
    case "processing":
      return "brand"
    case "parsed":
      return "positive"
    case "failed":
      return "negative"
  }
}

interface ClassificationCopy {
  readonly label: string
  readonly tone: ImportTone
  readonly summary: string
}

export const CLASSIFICATION_COPY: Record<
  ImportClassificationStatus,
  ClassificationCopy
> = {
  not_started: {
    label: "Not started",
    tone: "ghost",
    summary: "Transactions are categorized once the files have been read.",
  },
  waiting: {
    label: "Waiting",
    tone: "ghost",
    summary: "Transactions are categorized once every file has been read.",
  },
  running: {
    label: "Categorizing",
    tone: "brand",
    summary: "Sverto is working out the type and category of each transaction.",
  },
  classified: {
    label: "Done",
    tone: "positive",
    summary:
      "Each transaction has a type. A category is filled in only where Sverto is confident.",
  },
  failed: {
    label: "Failed",
    tone: "negative",
    summary: "The transactions could not be categorized.",
  },
}

interface VerbCopy {
  readonly label: string
  readonly tone: ImportTone
}

const KIND_NOTE: Record<ImportItemKind, string | null> = {
  movement: null,
  starting_balance:
    "Starting balance: added before the statement's first row so that its first balance matches your ledger.",
  joined_transfer:
    "Moved between your accounts: the row going out and the row coming in, imported as one transfer.",
  adjustment:
    "Adjustment: closes a difference between two statement balances that nothing in your ledger explains. Left out unless you add it.",
}

export interface SourceCopy {
  readonly reconciling: string
  readonly linkingFootnote: string
  readonly deleted: string
  readonly kindNote: Record<ImportItemKind, string | null>
  readonly startingBalance: string
  readonly startingBalancesFrom: string
  readonly balancesOff: string
  readonly noBalance: string
  readonly historyBefore: string
  readonly notListed: string
  readonly mayBookLater: string
  readonly nothingToCompare: string
}

export const FILE_SOURCE_COPY: SourceCopy = {
  reconciling: SESSION_STATUS_COPY.reconciling.summary,
  linkingFootnote:
    "Accounts are linked after the files are read. Nothing is written to your ledger until you approve the proposal.",
  deleted: "The import and the files uploaded to it are deleted.",
  kindNote: KIND_NOTE,
  startingBalance:
    "The statement's first balance needs this before its first row, on top of what your ledger already holds. Add it, leave it out, or upload the statement before this one.",
  startingBalancesFrom: "from the statements",
  balancesOff: "won't match the statements' balances.",
  noBalance: "No statement gives a balance to check against.",
  historyBefore: "from history before this statement.",
  notListed: "In your ledger but not on this statement",
  mayBookLater:
    "In your ledger, dated in the statement's last days: the bank may book these on the next statement",
  nothingToCompare: "the statements give nothing to compare with.",
}

export const BANK_SOURCE_COPY: SourceCopy = {
  reconciling: "Comparing the transactions from your bank with your ledger.",
  linkingFootnote:
    "The bank accounts start syncing once you apply this import. Nothing is written to your ledger until you approve the proposal.",
  deleted: "The import is deleted.",
  kindNote: {
    ...KIND_NOTE,
    starting_balance:
      "Starting balance: added before the first transaction from your bank so that your bank's balance matches your ledger.",
  },
  startingBalance:
    "Your bank's balance needs this before the first transaction from your bank, on top of what your ledger already holds. Add it or leave it out.",
  startingBalancesFrom: "from your bank",
  balancesOff: "won't match your bank's balances.",
  noBalance: "Your bank gives no balance to check against.",
  historyBefore: "from history before the transactions from your bank.",
  notListed: "In your ledger but not in the transactions from your bank",
  mayBookLater:
    "In your ledger, dated in the last few days: your bank may not have booked these yet",
  nothingToCompare: "your bank gives nothing to compare with.",
}

export const AMEND_REFUSAL_NOTE: Record<
  NonNullable<IdentifiableStagedTransaction["amend_refusal"]>,
  string
> = {
  target_gone:
    "This correction can't be made: the ledger transaction it matched is gone. Undo the pin to decide again.",
  not_one_entry:
    "This correction can't be made: the ledger transaction doesn't have exactly one entry on this account and asset. Undo the pin to decide again.",
  unreadable:
    "This correction can't be made: the ledger transaction can't be read. Undo the pin to decide again.",
}

export const VERB_COPY: Record<ImportItemVerb, VerbCopy> = {
  add: { label: "Add", tone: "positive" },
  amend: { label: "Amend", tone: "brand" },
  delete: { label: "Delete", tone: "negative" },
  skip: { label: "Skip", tone: "ghost" },
}
