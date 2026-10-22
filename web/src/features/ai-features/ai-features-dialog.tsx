import type { NormalizedError } from "@/lib/errors"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"

import {
  AI_FEATURES_FOOTER,
  AI_FEATURES_INTRO,
  AI_FEATURES_PROMPT_TITLE,
  NOT_NOW,
  TURN_ON,
  TURNING_ON,
} from "./copy"
import { AiFeaturesSharedData, PrivacyPolicyLink } from "./disclosure"

export function AiFeaturesDialog({
  open,
  isTurningOn,
  error,
  onTurnOn,
  onDecline,
}: {
  open: boolean
  isTurningOn: boolean
  error: NormalizedError | null
  onTurnOn: () => void
  onDecline: () => void
}) {
  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !isTurningOn) onDecline()
      }}
    >
      <AlertDialogContent className="rounded-sheet border border-border-strong bg-surface sm:max-w-md">
        <AlertDialogHeader>
          <AlertDialogTitle className="text-[14px] leading-[1.3] font-semibold">
            {AI_FEATURES_PROMPT_TITLE}
          </AlertDialogTitle>
          <AlertDialogDescription className="text-[12px] leading-[1.6] text-pretty text-ink-2 sm:text-left">
            {AI_FEATURES_INTRO}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AiFeaturesSharedData />
        <p className="text-[11.5px] leading-[1.6] text-pretty text-ink-3">
          {AI_FEATURES_FOOTER} <PrivacyPolicyLink />
        </p>
        {error === null ? null : (
          <p
            role="alert"
            className="rounded-md border border-negative-dim bg-surface-2 px-[13px] py-[11px] text-[11.5px] leading-[1.5] text-pretty text-negative"
          >
            {error.message}
          </p>
        )}
        <AlertDialogFooter>
          <AlertDialogCancel disabled={isTurningOn}>
            {NOT_NOW}
          </AlertDialogCancel>
          <AlertDialogAction disabled={isTurningOn} onClick={onTurnOn}>
            {isTurningOn ? TURNING_ON : TURN_ON}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
