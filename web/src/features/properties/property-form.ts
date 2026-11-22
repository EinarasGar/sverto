import { isSameDay } from "date-fns"

import type { CreatePropertyRequest, PropertyKind } from "@/api"
import {
  NAME_REQUIRED,
  parseSharePercent,
  sharePercentToFraction,
} from "@/features/accounts/account-form"
import { parseEditorDate } from "@/features/transactions/editor/date-input"
import { parseMoney } from "@/features/transactions/editor/money"
import { interpretFieldMessage, normalizeError } from "@/lib/errors"

export const VALUE_REQUIRED = "Enter what it is worth today, above zero."
export const PRICE_REQUIRED = "Enter what you paid, above zero."
export const DATE_UNREADABLE =
  "That date could not be read. Try 2021-03-01 or “30 days ago”."
export const DATE_IN_FUTURE = "Pick today or an earlier date."
export const DATE_REQUIRED = "Enter the day you bought it."

export interface PropertyFormState {
  readonly kind: PropertyKind
  readonly name: string
  readonly currencyAssetId: number | null
  readonly value: string
  readonly sharePercent: string
  readonly boughtOn: string
  readonly price: string
  readonly loanAccountIds: readonly string[]
}

export type PropertyFieldErrors = Partial<Record<string, string>>

export interface PropertyFormValidation {
  readonly payload: CreatePropertyRequest | null
  readonly fieldErrors: PropertyFieldErrors
}

export function emptyPropertyForm(
  currencyAssetId: number | null
): PropertyFormState {
  return {
    kind: "real_estate",
    name: "",
    currencyAssetId,
    value: "",
    sharePercent: "100",
    boughtOn: "today",
    price: "",
    loanAccountIds: [],
  }
}

export function parsePositiveAmount(text: string): number | null {
  const amount = parseMoney(text)
  return amount !== null && amount > 0 ? amount : null
}

export function isAfterNow(dateSeconds: number, now: Date): boolean {
  return dateSeconds * 1000 > now.getTime()
}

export function isBoughtBeforeToday(boughtOn: string, now: Date): boolean {
  const date = parseEditorDate(boughtOn, now).date
  return date !== null && !isSameDay(date * 1000, now) && !isAfterNow(date, now)
}

export function validatePropertyForm(
  state: PropertyFormState,
  now: Date
): PropertyFormValidation {
  const fieldErrors: PropertyFieldErrors = {}

  const name = state.name.trim()
  if (name === "") fieldErrors.name = NAME_REQUIRED

  if (state.currencyAssetId === null) {
    fieldErrors.currency_asset_id = "Pick the currency you value it in."
  }

  const share = parseSharePercent(state.sharePercent)
  if (share.error !== null) fieldErrors.ownership_share = share.error

  const boughtOn = parseEditorDate(state.boughtOn, now).date
  if (state.boughtOn.trim() === "") {
    fieldErrors["purchase.date"] = DATE_REQUIRED
  } else if (boughtOn === null) {
    fieldErrors["purchase.date"] = DATE_UNREADABLE
  } else if (isAfterNow(boughtOn, now)) {
    fieldErrors["purchase.date"] = DATE_IN_FUTURE
  }

  const price = parsePositiveAmount(state.price)
  if (price === null) fieldErrors["purchase.price"] = PRICE_REQUIRED

  const earlier = isBoughtBeforeToday(state.boughtOn, now)
  const value = earlier ? parsePositiveAmount(state.value) : price
  if (earlier && value === null) fieldErrors.value = VALUE_REQUIRED

  if (
    Object.keys(fieldErrors).length > 0 ||
    boughtOn === null ||
    price === null ||
    value === null ||
    share.percent === null ||
    state.currencyAssetId === null
  ) {
    return { payload: null, fieldErrors }
  }

  return {
    payload: {
      kind: state.kind,
      name,
      currency_asset_id: state.currencyAssetId,
      ownership_share: sharePercentToFraction(share.percent),
      value,
      ...(earlier ? { purchase: { date: boughtOn, price } } : {}),
      loan_account_ids: [...state.loanAccountIds],
    },
    fieldErrors: {},
  }
}

const FORM_FIELDS = new Set([
  "name",
  "currency_asset_id",
  "value",
  "ownership_share",
  "purchase",
  "purchase.date",
  "purchase.price",
])

export interface PropertyServerErrors {
  readonly fieldErrors: PropertyFieldErrors
  readonly formError: string | null
}

export function propertyServerErrors(error: unknown): PropertyServerErrors {
  const normalized = normalizeError(error)
  if (normalized.kind !== "validation" || normalized.fieldErrors.length === 0) {
    return { fieldErrors: {}, formError: normalized.message }
  }
  const fieldErrors: PropertyFieldErrors = {}
  const unplaced: string[] = []
  for (const fieldError of normalized.fieldErrors) {
    const message = interpretFieldMessage(fieldError.message).message
    if (FORM_FIELDS.has(fieldError.field)) {
      fieldErrors[fieldError.field] ??= message
    } else {
      unplaced.push(message)
    }
  }
  return {
    fieldErrors,
    formError: unplaced.length === 0 ? null : unplaced.join(" "),
  }
}
