import { Suspense, useState } from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useNavigate } from "@tanstack/react-router"

import { UsersApiFactory } from "@/api"
import { authMeQueryKey, useAuth, useAuthMe, useUserId } from "@/auth"
import { api } from "@/lib/api"
import type { AssetRef } from "@/lib/domain/refs"
import { AuthShell } from "@/components/layout/auth-shell"
import { ErrorStateFor } from "@/components/layout/error-states"
import { CURRENT_ONBOARDING_VERSION } from "@/components/layout/onboarding"
import { LoadingState, SkeletonRows } from "@/components/states/loading-state"
import { useSetAiFeatures } from "@/features/ai-features"
import { BRAND_FOOT } from "@/features/auth-screens"

import { AiFeaturesStep } from "./ai-features-step"
import { BaseCurrencyStep } from "./base-currency-step"
import { CURRENCY_SAVED_NOTE } from "./copy"
import type { StartPath } from "./start-paths"
import { StartStep } from "./start-step"
import { StepTracker } from "./step-tracker"
import {
  buildTracker,
  resumeStep,
  stepLabel,
  stepsAfterVersion,
  type OnboardingStep,
} from "./steps"
import { WelcomeStep } from "./welcome-step"

type Destination = () => Promise<void>

export function OnboardingWizard() {
  const userId = useUserId()
  const { baseCurrency } = useAuth()
  const me = useAuthMe(true)
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  const [steps] = useState(() =>
    stepsAfterVersion(
      baseCurrency === null ? 0 : (me.data?.onboarding_version ?? 0)
    )
  )
  const [step, setStep] = useState<OnboardingStep>(() =>
    baseCurrency === null ? "welcome" : resumeStep(steps)
  )

  const refreshIdentity = () =>
    queryClient.invalidateQueries({ queryKey: authMeQueryKey })

  const finish = useMutation({
    mutationFn: async (go: Destination) => {
      await api(UsersApiFactory).postOnboarding(userId, {
        version: CURRENT_ONBOARDING_VERSION,
      })
      await refreshIdentity()
      await go()
    },
    meta: { errorContext: "Setup could not be completed" },
  })

  const advanceFrom = (from: OnboardingStep) => {
    const next = steps[steps.indexOf(from) + 1]
    if (next === undefined) {
      finish.mutate(() => navigate({ to: "/" }))
      return
    }
    setStep(next)
  }

  const saveBaseCurrency = useMutation({
    mutationFn: async (asset: AssetRef) => {
      await api(UsersApiFactory).postBaseAsset(userId, {
        asset_id: asset.assetId,
      })
      await refreshIdentity()
    },
    onSuccess: () => {
      advanceFrom("currency")
    },
    meta: { errorContext: "Your base currency could not be saved" },
  })

  const saveAiFeatures = useSetAiFeatures(userId)

  const goTo: Record<StartPath["id"], Destination> = {
    transaction: () => navigate({ to: "/transactions" }),
    bank: () =>
      navigate({ to: "/settings", search: { section: "connections" } }),
    receipt: () =>
      navigate({ to: "/transactions", search: { mode: "review" } }),
  }

  const tracker = buildTracker(steps, step, baseCurrency)
  const eyebrow = stepLabel(steps, step)
  const previousStep = steps[steps.indexOf(step) - 1]
  const back =
    previousStep === undefined
      ? {}
      : {
          onBack: () => {
            setStep(previousStep)
          },
        }

  return (
    <AuthShell aside={<StepTracker steps={tracker} />} brandFoot={BRAND_FOOT}>
      {saveBaseCurrency.isError ? (
        <div className="mb-[18px]">
          <ErrorStateFor
            error={saveBaseCurrency.error}
            onRetry={() => saveBaseCurrency.reset()}
          />
        </div>
      ) : null}
      {saveAiFeatures.isError ? (
        <div className="mb-[18px]">
          <ErrorStateFor
            error={saveAiFeatures.error}
            onRetry={() => saveAiFeatures.reset()}
          />
        </div>
      ) : null}
      {finish.isError ? (
        <div className="mb-[18px]">
          <ErrorStateFor error={finish.error} onRetry={() => finish.reset()} />
        </div>
      ) : null}

      {previousStep === "currency" && baseCurrency ? (
        <p className="mb-[18px] text-[11.5px] leading-[1.5] text-pretty text-ink-3">
          {`${baseCurrency} · ${CURRENCY_SAVED_NOTE}`}
        </p>
      ) : null}

      {step === "welcome" ? (
        <WelcomeStep
          eyebrow={eyebrow}
          onContinue={() => {
            advanceFrom("welcome")
          }}
        />
      ) : null}

      {step === "currency" ? (
        <Suspense
          fallback={
            <LoadingState label="Loading currencies">
              <SkeletonRows count={5} height={40} />
            </LoadingState>
          }
        >
          <BaseCurrencyStep
            eyebrow={eyebrow}
            onConfirm={(asset) => saveBaseCurrency.mutate(asset)}
            isSubmitting={saveBaseCurrency.isPending}
            {...back}
          />
        </Suspense>
      ) : null}

      {step === "ai" ? (
        <AiFeaturesStep
          eyebrow={eyebrow}
          onChoose={(enabled) => {
            saveAiFeatures.mutate(enabled, {
              onSuccess: () => {
                advanceFrom("ai")
              },
            })
          }}
          isSubmitting={saveAiFeatures.isPending || finish.isPending}
          {...back}
        />
      ) : null}

      {step === "start" ? (
        <StartStep
          eyebrow={eyebrow}
          onChoose={(id) => finish.mutate(goTo[id])}
          onSkip={() => finish.mutate(() => navigate({ to: "/" }))}
          isSubmitting={finish.isPending}
          {...back}
        />
      ) : null}
    </AuthShell>
  )
}
