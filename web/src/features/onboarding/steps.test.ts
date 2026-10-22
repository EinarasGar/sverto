import { describe, expect, it } from "vitest"

import {
  buildTracker,
  ONBOARDING_STEPS,
  resumeStep,
  stepLabel,
  stepsAfterVersion,
} from "./steps"

describe("buildTracker", () => {
  it("marks exactly one step as current and everything before it as done", () => {
    const tracker = buildTracker(ONBOARDING_STEPS, "currency", null)

    expect(tracker.map((step) => step.state)).toEqual([
      "done",
      "now",
      "todo",
      "todo",
    ])
    expect(tracker.filter((step) => step.state === "now")).toHaveLength(1)
  })

  it("ticks a completed step rather than numbering it", () => {
    expect(
      buildTracker(ONBOARDING_STEPS, "start", null).map((step) => step.ordinal)
    ).toEqual(["✓", "✓", "✓", "4"])
  })

  it("shows the chosen currency once it is saved, and never before", () => {
    expect(buildTracker(ONBOARDING_STEPS, "currency", "GBP")[1]?.body).toBe(
      "The one everything converts into."
    )
    expect(buildTracker(ONBOARDING_STEPS, "ai", "GBP")[1]?.body).toBe("GBP")
    expect(buildTracker(ONBOARDING_STEPS, "ai", null)[1]?.body).toBe(
      "The one everything converts into."
    )
  })

  it("lists only the steps it is given", () => {
    expect(
      buildTracker(["ai"], "ai", "GBP").map((step) => [
        step.title,
        step.ordinal,
      ])
    ).toEqual([["AI features", "1"]])
  })
})

describe("stepsAfterVersion", () => {
  it("walks a brand-new account through every step", () => {
    expect(stepsAfterVersion(0)).toEqual(ONBOARDING_STEPS)
  })

  it("shows an account that finished version 1 only what version 2 added", () => {
    expect(stepsAfterVersion(1)).toEqual(["ai"])
  })

  it("has nothing left for an account on the current version", () => {
    expect(stepsAfterVersion(2)).toEqual([])
  })
})

describe("resumeStep", () => {
  it("picks up after the base currency for an account that already has one", () => {
    expect(resumeStep(ONBOARDING_STEPS)).toBe("ai")
    expect(resumeStep(["ai"])).toBe("ai")
  })
})

describe("stepLabel", () => {
  it("counts within the steps this account actually sees", () => {
    expect(stepLabel(ONBOARDING_STEPS, "welcome")).toBe("Step 1 of 4")
    expect(stepLabel(ONBOARDING_STEPS, "start")).toBe("Step 4 of 4")
    expect(stepLabel(["ai"], "ai")).toBe("Step 1 of 1")
  })
})
