import { describe, expect, it } from "vitest"

import type { PropertyFormState } from "./property-form"
import {
  DATE_IN_FUTURE,
  DATE_REQUIRED,
  emptyPropertyForm,
  isBoughtBeforeToday,
  PRICE_REQUIRED,
  propertyServerErrors,
  validatePropertyForm,
  VALUE_REQUIRED,
} from "./property-form"

const NOW = new Date(2026, 10, 21, 12)

function form(overrides: Partial<PropertyFormState> = {}): PropertyFormState {
  return {
    ...emptyPropertyForm(45),
    name: "Flat 12",
    price: "296,000",
    ...overrides,
  }
}

describe("validatePropertyForm", () => {
  it("defaults to bought today and sends the price as its only value", () => {
    expect(emptyPropertyForm(45).boughtOn).toBe("today")
    expect(validatePropertyForm(form({ value: "junk" }), NOW)).toEqual({
      payload: {
        kind: "real_estate",
        name: "Flat 12",
        currency_asset_id: 45,
        ownership_share: 1,
        value: 296000,
        loan_account_ids: [],
      },
      fieldErrors: {},
    })
  })

  it("treats today's date typed out as bought today", () => {
    const { payload } = validatePropertyForm(
      form({ boughtOn: "2026-11-21" }),
      NOW
    )
    expect(payload?.purchase).toBeUndefined()
    expect(payload?.value).toBe(296000)
  })

  it("sends an earlier purchase at the start of that day beside today's value", () => {
    const { payload } = validatePropertyForm(
      form({
        sharePercent: "50",
        boughtOn: "2021-03-01",
        value: "328,000",
        loanAccountIds: ["m1"],
      }),
      NOW
    )
    expect(payload?.ownership_share).toBe(0.5)
    expect(payload?.purchase).toEqual({
      date: new Date(2021, 2, 1).getTime() / 1000,
      price: 296000,
    })
    expect(payload?.value).toBe(328000)
    expect(payload?.loan_account_ids).toEqual(["m1"])
  })

  it("asks for today's value only when it was bought earlier", () => {
    const earlier = validatePropertyForm(form({ boughtOn: "2021-03-01" }), NOW)
    expect(earlier.payload).toBeNull()
    expect(earlier.fieldErrors.value).toBe(VALUE_REQUIRED)

    const zero = validatePropertyForm(
      form({ boughtOn: "2021-03-01", value: "0" }),
      NOW
    )
    expect(zero.fieldErrors.value).toBe(VALUE_REQUIRED)
  })

  it("requires the day it was bought and the price", () => {
    const { payload, fieldErrors } = validatePropertyForm(
      form({ boughtOn: " ", price: "0" }),
      NOW
    )
    expect(payload).toBeNull()
    expect(fieldErrors["purchase.date"]).toBe(DATE_REQUIRED)
    expect(fieldErrors["purchase.price"]).toBe(PRICE_REQUIRED)
  })

  it("refuses a purchase date after today", () => {
    const { payload, fieldErrors } = validatePropertyForm(
      form({ boughtOn: "2026-11-22", value: "328000" }),
      NOW
    )
    expect(payload).toBeNull()
    expect(fieldErrors["purchase.date"]).toBe(DATE_IN_FUTURE)
  })
})

describe("isBoughtBeforeToday", () => {
  it("is true only for a readable day before today", () => {
    expect(isBoughtBeforeToday("2025-03-01", NOW)).toBe(true)
    expect(isBoughtBeforeToday("yesterday", NOW)).toBe(true)
    expect(isBoughtBeforeToday("today", NOW)).toBe(false)
    expect(isBoughtBeforeToday("2026-11-21", NOW)).toBe(false)
    expect(isBoughtBeforeToday("2026-11-22", NOW)).toBe(false)
    expect(isBoughtBeforeToday("not a date", NOW)).toBe(false)
  })
})

describe("propertyServerErrors", () => {
  it("puts a field error on its field and a conflict above the form", () => {
    const validation = Object.assign(new Error("rejected"), {
      isAxiosError: true,
      response: {
        status: 422,
        data: {
          error_type: "ValidationError",
          message: "One or more fields failed validation.",
          errors: [
            { field: "purchase.date", message: "Must not be in the future." },
          ],
        },
      },
    })
    expect(propertyServerErrors(validation).fieldErrors["purchase.date"]).toBe(
      "Must not be in the future."
    )

    const conflict = Object.assign(new Error("rejected"), {
      isAxiosError: true,
      response: {
        status: 409,
        data: {
          error_type: "Conflict",
          message: "An account named Flat 12 already exists.",
          errors: [],
        },
      },
    })
    expect(propertyServerErrors(conflict)).toEqual({
      fieldErrors: {},
      formError: "An account named Flat 12 already exists.",
    })
  })
})
