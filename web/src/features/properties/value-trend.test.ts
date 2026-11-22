import { describe, expect, it } from "vitest"

import { valueTrend } from "./value-trend"

const DAY = 86_400

function valuation(day: number, value: number) {
  return { timestamp: day * DAY, value }
}

function expectValuesAt(
  line: readonly { value: number }[],
  expected: readonly (readonly [number, number])[]
) {
  for (const [k, value] of expected) {
    expect(Math.abs(line[k]!.value - value)).toBeLessThan(0.01)
  }
}

describe("valueTrend", () => {
  it("spreads a cliff between two close valuations", () => {
    const line = valueTrend(
      [valuation(0, 300000), valuation(100, 300000), valuation(103, 330000)],
      365 * DAY,
      330000
    )

    expect(line).toHaveLength(120)
    expect([line[0]!.timestamp, line[119]!.timestamp]).toEqual([0, 365 * DAY])
    expectValuesAt(line, [
      [0, 300000],
      [30, 311321.9947],
      [60, 329887.1903],
      [90, 329993.993],
      [119, 330000],
    ])
    const steps = line.slice(1).map((point, k) => point.value - line[k]!.value)
    expect(steps.every((step) => step >= 0)).toBe(true)
    expect(Math.max(...steps)).toBeLessThan(1300)
  })

  it("ends at today's value for a real history", () => {
    const line = valueTrend(
      [valuation(0, 296000), valuation(341, 310000), valuation(397, 328000)],
      400 * DAY,
      328000
    )

    expectValuesAt(line, [
      [0, 296000],
      [30, 301136.3605],
      [60, 307311.9406],
      [90, 314033.3317],
      [119, 328000],
    ])
  })

  it("carries the last valuation when today's value is unknown", () => {
    const line = valueTrend(
      [valuation(0, 100), valuation(10, 120)],
      20 * DAY,
      null
    )
    expectValuesAt(line, [[119, 120]])
  })

  it("is a single point for a valuation made now, and empty with none", () => {
    expect(valueTrend([], 5 * DAY, 1)).toEqual([])
    const line = valueTrend([valuation(5, 18500)], 5 * DAY, 18500)
    expect(line).toHaveLength(1)
    expectValuesAt(line, [[0, 18500]])
  })
})
