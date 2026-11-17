import { describe, expect, it } from "vitest"

import { accountPrefill } from "./link-copy"

describe("linking copy", () => {
  it("prefills an identifier only when the name looks like one", () => {
    expect(accountPrefill("LT12 1000 0111 0100 1000").identifier).toEqual({
      kind: "iban",
      value: "LT121000011101001000",
    })
    expect(accountPrefill("31926819").identifier).toEqual({
      kind: "account_number",
      value: "31926819",
    })
    expect(accountPrefill("Card ****1234").identifier).toEqual({
      kind: "card_last4",
      value: "1234",
    })
    expect(accountPrefill("Current")).toEqual({ name: "Current" })
  })
})
