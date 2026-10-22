import { AI_FEATURES_TITLE } from "@/features/ai-features"

import {
  AI_STEP_BODY,
  CURRENCY_STEP_BODY,
  CURRENCY_STEP_TITLE,
  START_STEP_BODY,
  START_STEP_TITLE,
  STEP_LABEL,
  WELCOME_STEP_BODY,
  WELCOME_STEP_TITLE,
} from "./copy"

export const ONBOARDING_STEPS = ["welcome", "currency", "ai", "start"] as const

export type OnboardingStep = (typeof ONBOARDING_STEPS)[number]

export type StepState = "done" | "now" | "todo"

export interface TrackerStep {
  id: OnboardingStep
  ordinal: string
  title: string
  body: string
  state: StepState
}

const INTRODUCED_IN_VERSION: Record<OnboardingStep, number> = {
  welcome: 1,
  currency: 1,
  ai: 2,
  start: 1,
}

const STEP_COPY: Record<OnboardingStep, { title: string; body: string }> = {
  welcome: { title: WELCOME_STEP_TITLE, body: WELCOME_STEP_BODY },
  currency: { title: CURRENCY_STEP_TITLE, body: CURRENCY_STEP_BODY },
  ai: { title: AI_FEATURES_TITLE, body: AI_STEP_BODY },
  start: { title: START_STEP_TITLE, body: START_STEP_BODY },
}

export function stepsAfterVersion(
  completedVersion: number
): readonly OnboardingStep[] {
  return ONBOARDING_STEPS.filter(
    (step) => INTRODUCED_IN_VERSION[step] > completedVersion
  )
}

export function resumeStep(steps: readonly OnboardingStep[]): OnboardingStep {
  return (
    steps.find((step) => step !== "welcome" && step !== "currency") ?? "start"
  )
}

export function stepLabel(
  steps: readonly OnboardingStep[],
  step: OnboardingStep
): string {
  return STEP_LABEL(steps.indexOf(step) + 1, steps.length)
}

export function buildTracker(
  steps: readonly OnboardingStep[],
  current: OnboardingStep,
  chosenCurrency: string | null
): TrackerStep[] {
  const currentIndex = steps.indexOf(current)

  return steps.map((id, index) => {
    const state: StepState =
      index < currentIndex ? "done" : index === currentIndex ? "now" : "todo"
    const copy = STEP_COPY[id]
    const body =
      id === "currency" && state === "done" && chosenCurrency !== null
        ? chosenCurrency
        : copy.body

    return {
      id,
      ordinal: state === "done" ? "✓" : String(index + 1),
      title: copy.title,
      body,
      state,
    }
  })
}
