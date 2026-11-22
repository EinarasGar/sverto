import { describe, expect, it } from "vitest"

import { valuationEditSteps } from "./valuation-steps"

const OLD = { date: 1_790_000_000, value: 310000 }

describe("valuationEditSteps", () => {
  it("adds first when the valuation moves, so a failed add keeps the old one", () => {
    const to = { date: 1_780_000_000, value: 305000 }
    expect(valuationEditSteps(OLD, to)).toEqual([
      { kind: "add", valuation: to },
      { kind: "delete", date: OLD.date },
    ])
  })

  it("frees the slot first when the time stays within a second", () => {
    for (const date of [OLD.date, OLD.date + 1, OLD.date - 1]) {
      const to = { date, value: 315000 }
      expect(valuationEditSteps(OLD, to)).toEqual([
        { kind: "delete", date: OLD.date },
        { kind: "add", valuation: to },
      ])
    }
  })
})
