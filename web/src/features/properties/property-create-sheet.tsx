import { useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useNavigate } from "@tanstack/react-router"

import type { CreatePropertyRequest, PropertyKind } from "@/api"
import { useUserId } from "@/auth"
import { AdaptiveSheet } from "@/components/layout/adaptive-sheet"
import type { PickerOption } from "@/components/primitives"
import { EntityPicker, SegmentedControl } from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
  FieldTitle,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { toast } from "@/components/ui/toast"
import { accountsQueryOptions } from "@/features/accounts/api"
import { currencyAssetsQueryOptions } from "@/features/onboarding/currency-assets"
import { useRequiredBaseAssetId } from "@/features/portfolio/api"
import { parseEditorDate } from "@/features/transactions/editor/date-input"
import type { AssetRef } from "@/lib/domain/refs"
import { assetDisplayName, assetLabel } from "@/lib/domain/refs"

import { propertiesQueryOptions, useCreateProperty } from "./api"
import { linkableLoans } from "./loans"
import { ADD_PROPERTY, PROPERTY_KIND_LABELS } from "./presentation"
import type { PropertyFieldErrors, PropertyFormState } from "./property-form"
import {
  emptyPropertyForm,
  isBoughtBeforeToday,
  propertyServerErrors,
  validatePropertyForm,
} from "./property-form"
import { ShareField } from "./share-field"

const FORM_ID = "property-create-form"
const LABEL_CLASS = "text-[11.5px] leading-none font-semibold text-ink-2"
const HINT_CLASS = "text-[11.5px] leading-[1.5] text-pretty text-ink-3"
const ERROR_CLASS = "text-[11.5px] leading-[1.5]"

const KIND_OPTIONS = [
  { value: "real_estate", label: PROPERTY_KIND_LABELS.real_estate },
  { value: "vehicle", label: PROPERTY_KIND_LABELS.vehicle },
] as const satisfies readonly { value: PropertyKind; label: string }[]

const NAME_PLACEHOLDERS: Record<PropertyKind, string> = {
  real_estate: "Flat 12, Wren House",
  vehicle: "Golf GTI",
}

function currencyOption(asset: AssetRef): PickerOption {
  const label = assetLabel(asset)
  const name = assetDisplayName(asset)
  return {
    value: String(asset.assetId),
    label,
    ...(name === label ? {} : { subLabel: name, keywords: [name] }),
  }
}

function FormSkeleton() {
  return (
    <div className="flex flex-col gap-5">
      {[0, 1, 2, 3, 4].map((row) => (
        <div key={row} className="flex flex-col gap-2">
          <Skeleton className="h-3 w-24" />
          <Skeleton className="h-10 w-full" />
        </div>
      ))}
    </div>
  )
}

function Message({ text }: { text: string | undefined }) {
  return text === undefined ? null : (
    <FieldError className={ERROR_CLASS}>{text}</FieldError>
  )
}

function PropertyCreateForm({
  serverErrors,
  formError,
  onDirty,
  onSubmit,
}: {
  serverErrors: PropertyFieldErrors
  formError: string | null
  onDirty: () => void
  onSubmit: (body: CreatePropertyRequest) => void
}) {
  const userId = useUserId()
  const baseAssetId = useRequiredBaseAssetId()
  const currencies = useQuery(currencyAssetsQueryOptions())
  const accounts = useQuery(accountsQueryOptions(userId))
  const properties = useQuery(propertiesQueryOptions(userId))
  const [form, setForm] = useState<PropertyFormState>(() =>
    emptyPropertyForm(baseAssetId)
  )
  const [submitted, setSubmitted] = useState(false)

  const currencyOptions = useMemo(
    () => (currencies.data ?? []).map(currencyOption),
    [currencies.data]
  )
  const loans = useMemo(
    () =>
      accounts.data === undefined || properties.data === undefined
        ? []
        : linkableLoans(accounts.data, properties.data),
    [accounts.data, properties.data]
  )

  if (currencies.isPending || accounts.isPending || properties.isPending) {
    return <FormSkeleton />
  }

  const now = new Date()
  const validation = validatePropertyForm(form, now)
  const errors: PropertyFieldErrors = {
    ...(submitted ? validation.fieldErrors : {}),
    ...serverErrors,
  }
  const boughtOn = parseEditorDate(form.boughtOn, now)
  const boughtEarlier = isBoughtBeforeToday(form.boughtOn, now)

  function patch(next: Partial<PropertyFormState>) {
    setForm((previous) => ({ ...previous, ...next }))
    onDirty()
  }

  return (
    <form
      id={FORM_ID}
      noValidate
      className="flex flex-col gap-5"
      onSubmit={(event) => {
        event.preventDefault()
        setSubmitted(true)
        if (validation.payload !== null) onSubmit(validation.payload)
      }}
    >
      {formError === null ? null : (
        <div
          role="alert"
          className="rounded-md border border-negative-dim bg-surface-2 px-[13px] py-[11px] text-[11.5px] leading-[1.5] text-pretty text-negative"
        >
          {formError}
        </div>
      )}

      <Field>
        <FieldTitle className={LABEL_CLASS}>What it is</FieldTitle>
        <SegmentedControl
          label="Kind of property"
          value={form.kind}
          options={KIND_OPTIONS}
          stretch
          onValueChange={(kind) => {
            patch({ kind })
          }}
        />
      </Field>

      <Field data-invalid={errors.name !== undefined}>
        <FieldLabel className={LABEL_CLASS} htmlFor="property-name">
          Name
        </FieldLabel>
        <Input
          id="property-name"
          value={form.name}
          autoComplete="off"
          placeholder={NAME_PLACEHOLDERS[form.kind]}
          {...(errors.name === undefined ? {} : { "aria-invalid": true })}
          onChange={(event) => {
            patch({ name: event.target.value })
          }}
        />
        <FieldDescription className={HINT_CLASS}>
          It gets its own account with this name, so it cannot match another
          account.
        </FieldDescription>
        <Message text={errors.name} />
      </Field>

      <Field data-invalid={errors.currency_asset_id !== undefined}>
        <FieldLabel className={LABEL_CLASS} htmlFor="property-currency">
          Currency
        </FieldLabel>
        <EntityPicker
          id="property-currency"
          value={
            form.currencyAssetId === null ? null : String(form.currencyAssetId)
          }
          placeholder="Search currencies"
          invalid={errors.currency_asset_id !== undefined}
          options={currencyOptions}
          onValueChange={(next) => {
            patch({ currencyAssetId: next === null ? null : Number(next) })
          }}
        />
        <FieldDescription className={HINT_CLASS}>
          The currency you value it in. Totals convert it to your base currency.
        </FieldDescription>
        <Message text={errors.currency_asset_id} />
      </Field>

      <Field data-invalid={errors.purchase !== undefined}>
        <FieldTitle className={LABEL_CLASS}>Purchase</FieldTitle>
        <div className="grid grid-cols-2 gap-3">
          <Field data-invalid={errors["purchase.date"] !== undefined}>
            <FieldLabel
              className="text-[11px] leading-none text-ink-3"
              htmlFor="property-bought-on"
            >
              Bought on
            </FieldLabel>
            <Input
              id="property-bought-on"
              value={form.boughtOn}
              autoComplete="off"
              placeholder="today"
              {...(errors["purchase.date"] === undefined
                ? {}
                : { "aria-invalid": true })}
              onChange={(event) => {
                patch({ boughtOn: event.target.value })
              }}
            />
            {boughtOn.label === null ? null : (
              <FieldDescription className={HINT_CLASS}>
                {boughtOn.label}
              </FieldDescription>
            )}
            <Message text={errors["purchase.date"]} />
          </Field>
          <Field data-invalid={errors["purchase.price"] !== undefined}>
            <FieldLabel
              className="text-[11px] leading-none text-ink-3"
              htmlFor="property-price"
            >
              Price paid
            </FieldLabel>
            <Input
              id="property-price"
              inputMode="decimal"
              autoComplete="off"
              placeholder="0.00"
              value={form.price}
              {...(errors["purchase.price"] === undefined
                ? {}
                : { "aria-invalid": true })}
              onChange={(event) => {
                patch({ price: event.target.value })
              }}
            />
            <Message text={errors["purchase.price"]} />
          </Field>
        </div>
        <FieldDescription className={HINT_CLASS}>
          {boughtEarlier
            ? "What you paid for the whole of it. Gains are measured from this price."
            : "What you paid for the whole of it. Bought today, so this is also what it is worth now."}
        </FieldDescription>
        <Message text={errors.purchase} />
      </Field>

      {boughtEarlier ? (
        <Field data-invalid={errors.value !== undefined}>
          <FieldLabel className={LABEL_CLASS} htmlFor="property-value">
            Value today
          </FieldLabel>
          <Input
            id="property-value"
            inputMode="decimal"
            autoComplete="off"
            placeholder="0.00"
            value={form.value}
            {...(errors.value === undefined ? {} : { "aria-invalid": true })}
            onChange={(event) => {
              patch({ value: event.target.value })
            }}
          />
          <FieldDescription className={HINT_CLASS}>
            The whole property, not your share. Update it from its page whenever
            you have a new figure.
          </FieldDescription>
          <Message text={errors.value} />
        </Field>
      ) : null}

      <ShareField
        id="property-share"
        value={form.sharePercent}
        error={errors.ownership_share}
        onChange={(sharePercent) => {
          patch({ sharePercent })
        }}
      />

      <Field>
        <FieldTitle className={LABEL_CLASS}>Loans (optional)</FieldTitle>
        {loans.length === 0 ? (
          <FieldDescription className={HINT_CLASS}>
            You have no mortgage or loan account that is not already linked to a
            property. Add one as an account, then link it from the
            property&rsquo;s page.
          </FieldDescription>
        ) : (
          <>
            <ul className="flex flex-col gap-[10px]">
              {loans.map((loan) => {
                const checked = form.loanAccountIds.includes(loan.accountId)
                const id = `property-loan-${loan.accountId}`
                return (
                  <li key={loan.accountId} className="flex items-center gap-3">
                    <Checkbox
                      id={id}
                      checked={checked}
                      onCheckedChange={(next) => {
                        patch({
                          loanAccountIds: next
                            ? [...form.loanAccountIds, loan.accountId]
                            : form.loanAccountIds.filter(
                                (accountId) => accountId !== loan.accountId
                              ),
                        })
                      }}
                    />
                    <label
                      htmlFor={id}
                      className="min-w-0 flex-1 text-[12.5px] leading-[1.35] font-medium"
                    >
                      {loan.name}
                      <span className="ms-2 text-[11px] font-normal text-ink-3">
                        {loan.accountTypeName}
                      </span>
                    </label>
                  </li>
                )
              })}
            </ul>
            <FieldDescription className={HINT_CLASS}>
              A linked loan leaves your accounts list and is subtracted from
              this property&rsquo;s row, which then shows your equity.
            </FieldDescription>
          </>
        )}
      </Field>
    </form>
  )
}

export function PropertyCreateSheet({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const userId = useUserId()
  const navigate = useNavigate()
  const create = useCreateProperty(userId)
  const [serverErrors, setServerErrors] = useState<PropertyFieldErrors>({})
  const [formError, setFormError] = useState<string | null>(null)

  function close() {
    setServerErrors({})
    setFormError(null)
    onOpenChange(false)
  }

  return (
    <AdaptiveSheet
      open={open}
      onOpenChange={(next) => {
        if (next) onOpenChange(true)
        else close()
      }}
      title={ADD_PROPERTY}
      width={440}
      footer={
        <>
          <span className="flex-1" />
          <Button
            type="button"
            variant="outline"
            size="lg"
            disabled={create.isPending}
            onClick={close}
          >
            Cancel
          </Button>
          <Button
            type="submit"
            form={FORM_ID}
            size="lg"
            disabled={create.isPending}
          >
            {create.isPending ? "Saving…" : ADD_PROPERTY}
          </Button>
        </>
      }
    >
      <PropertyCreateForm
        serverErrors={serverErrors}
        formError={formError}
        onDirty={() => {
          setServerErrors((previous) =>
            Object.keys(previous).length === 0 ? previous : {}
          )
          setFormError(null)
        }}
        onSubmit={(body) => {
          create.mutate(
            { body },
            {
              onSuccess: (propertyId) => {
                toast.add({
                  type: "success",
                  title: "Property added",
                  description: `${body.name} is on your accounts list.`,
                  actionProps: {
                    children: "Open",
                    onClick: () => {
                      void navigate({
                        to: "/properties/$propertyId",
                        params: { propertyId },
                      })
                    },
                  },
                })
                close()
              },
              onError: (error) => {
                const mapped = propertyServerErrors(error)
                setServerErrors(mapped.fieldErrors)
                setFormError(mapped.formError)
              },
            }
          )
        }}
      />
    </AdaptiveSheet>
  )
}
