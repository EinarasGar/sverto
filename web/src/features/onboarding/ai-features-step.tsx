import { Footnote } from "@/components/primitives/panel"
import { Button } from "@/components/ui/button"
import {
  AI_FEATURES_FOOTER,
  AI_FEATURES_INTRO,
  AI_FEATURES_TITLE,
  AiFeaturesSharedData,
  KEEP_AI_FEATURES_OFF,
  PrivacyPolicyLink,
  TURN_ON_AI_FEATURES,
} from "@/features/ai-features"

import { AI_STEP_PENDING } from "./copy"
import { StepFrame, StepNav } from "./step-layout"

export function AiFeaturesStep({
  eyebrow,
  onChoose,
  onBack,
  isSubmitting,
}: {
  eyebrow: string
  onChoose: (enabled: boolean) => void
  onBack?: () => void
  isSubmitting: boolean
}) {
  return (
    <StepFrame
      eyebrow={eyebrow}
      title={AI_FEATURES_TITLE}
      intro={AI_FEATURES_INTRO}
      onSubmit={(event) => {
        event.preventDefault()
        if (!isSubmitting) onChoose(true)
      }}
    >
      <AiFeaturesSharedData className="mt-6 rounded-panel border border-border px-4 py-[15px]" />

      <Footnote>
        {AI_FEATURES_FOOTER} <PrivacyPolicyLink />
      </Footnote>

      <StepNav
        {...(onBack ? { onBack } : {})}
        nextLabel={isSubmitting ? AI_STEP_PENDING : TURN_ON_AI_FEATURES}
        nextDisabled={isSubmitting}
        secondary={
          <Button
            type="button"
            variant="outline"
            size="lg"
            disabled={isSubmitting}
            onClick={() => {
              onChoose(false)
            }}
          >
            {KEEP_AI_FEATURES_OFF}
          </Button>
        }
      />
    </StepFrame>
  )
}
