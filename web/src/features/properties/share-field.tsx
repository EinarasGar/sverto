import {
  formatSharePercent,
  parseSharePercent,
} from "@/features/accounts/account-form"
import {
  formatSharePercentLabel,
  SHARE_ALL_MINE,
  SHARE_HALF,
} from "@/features/accounts/copy"
import { Button } from "@/components/ui/button"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"

function shareConsequence(raw: string): string | null {
  const { percent } = parseSharePercent(raw)
  if (percent === null) return null
  if (percent >= 100) return "It is all yours."
  return `Your accounts list and net worth count ${formatSharePercentLabel(percent)} of it. The figures on its page stay whole.`
}

export function ShareField({
  id,
  value,
  error,
  onChange,
}: {
  id: string
  value: string
  error: string | undefined
  onChange: (value: string) => void
}) {
  const consequence = shareConsequence(value)

  return (
    <Field data-invalid={error !== undefined}>
      <FieldLabel
        className="text-[11.5px] leading-none font-semibold text-ink-2"
        htmlFor={id}
      >
        Your share
      </FieldLabel>
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative w-[124px]">
          <Input
            id={id}
            className="pr-8 text-right font-mono tabular-nums"
            inputMode="decimal"
            autoComplete="off"
            value={value}
            {...(error === undefined ? {} : { "aria-invalid": true })}
            onChange={(event) => {
              onChange(event.target.value)
            }}
          />
          <span
            aria-hidden
            className="pointer-events-none absolute inset-y-0 right-[13px] flex items-center font-mono text-[12.5px] leading-none font-medium text-ink-3"
          >
            %
          </span>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => {
            onChange(formatSharePercent(1))
          }}
        >
          {SHARE_ALL_MINE}
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => {
            onChange(formatSharePercent(0.5))
          }}
        >
          {SHARE_HALF}
        </Button>
      </div>
      {consequence === null ? null : (
        <FieldDescription className="text-[11.5px] leading-[1.5] text-pretty text-ink-3">
          {consequence}
        </FieldDescription>
      )}
      {error === undefined ? null : (
        <FieldError className="text-[11.5px] leading-[1.5]">{error}</FieldError>
      )}
    </Field>
  )
}
