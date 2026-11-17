import {
  AccountIdentifierKind,
  type IdentifiableImportPendingTransaction,
} from "@/api"
import type { AccountPrefill } from "@/features/accounts"
import { countOf } from "@/lib/format"

export interface AskSubject {
  readonly kind: "account" | "asset"
  readonly name: string
}

export function questionAnchor(kind: AskSubject["kind"], name: string) {
  return `question-${kind}-${encodeURIComponent(name)}`
}

export function askSubject(askKey: string): AskSubject {
  if (askKey.startsWith("account_mapping:")) {
    const rest = askKey.slice("account_mapping:".length)
    return { kind: "account", name: rest.slice(rest.indexOf(":") + 1) }
  }
  return { kind: "asset", name: askKey.slice("asset_mapping:".length) }
}

const IBAN = /^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/
const CARD = /\*+\s*(\d{4})$/
const ACCOUNT_NUMBER = /^\d{4,34}$/

export function accountPrefill(raw: string): AccountPrefill {
  const compact = raw.replace(/\s+/g, "").toUpperCase()
  if (IBAN.test(compact)) {
    return {
      name: raw,
      identifier: { kind: AccountIdentifierKind.Iban, value: compact },
    }
  }
  const card = CARD.exec(raw)?.[1]
  if (card !== undefined) {
    return {
      name: raw,
      identifier: { kind: AccountIdentifierKind.CardLast4, value: card },
    }
  }
  if (ACCOUNT_NUMBER.test(compact)) {
    return {
      name: raw,
      identifier: { kind: AccountIdentifierKind.AccountNumber, value: compact },
    }
  }
  return { name: raw }
}

export function leaveOutConsequence(
  assetRef: string,
  waiting: readonly IdentifiableImportPendingTransaction[]
): string {
  const moving = waiting.filter((row) =>
    row.legs.some((leg) => leg.asset_ref === assetRef)
  )
  const alone = moving.filter((row) =>
    row.legs.every((leg) => leg.asset_ref === assetRef)
  ).length
  const beside = moving.length - alone
  const parts = [
    moving.length === 0
      ? `No row moves “${assetRef}”.`
      : `${countOf(moving.length, "row")} of this import ${moving.length === 1 ? "moves" : "move"} “${assetRef}”.`,
    alone > 0
      ? `${countOf(alone, "row")} ${alone === 1 ? "moves" : "move"} nothing else and will not be imported.`
      : null,
    beside > 0
      ? `On the other ${countOf(beside, "row")}, what was paid for it or received from it is recorded as a transfer.`
      : null,
    "Balances stated for it are not checked, and this answer can’t be changed afterwards.",
  ]
  return parts.filter((part) => part !== null).join(" ")
}
