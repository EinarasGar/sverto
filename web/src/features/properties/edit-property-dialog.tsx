import { useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { isSameDay } from "date-fns"

import { useUserId } from "@/auth"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { toast } from "@/components/ui/toast"
import {
  formatSharePercent,
  NAME_REQUIRED,
  parseSharePercent,
  sharePercentToFraction,
} from "@/features/accounts/account-form"
import {
  formatEditorDate,
  parseEditorDate,
} from "@/features/transactions/editor/date-input"

import type { Property } from "./api"
import {
  propertyPurchaseQueryOptions,
  useSetPropertyPurchaseDate,
  useUpdateProperty,
} from "./api"
import type { PropertyFieldErrors } from "./property-form"
import {
  DATE_IN_FUTURE,
  DATE_UNREADABLE,
  isAfterNow,
  propertyServerErrors,
} from "./property-form"
import { ShareField } from "./share-field"

const BOUGHT_ON_HINT =
  "Moves its purchase transaction, and the valuation on that day with it."

export function EditPropertyDialog({
  property,
  onOpenChange,
}: {
  property: Property
  onOpenChange: (open: boolean) => void
}) {
  const userId = useUserId()
  const update = useUpdateProperty(userId)
  const setPurchaseDate = useSetPropertyPurchaseDate(userId)
  const purchase = useQuery(propertyPurchaseQueryOptions(userId, property))
  const purchaseDate = purchase.data?.date ?? null
  const [name, setName] = useState(property.name)
  const [sharePercent, setSharePercent] = useState(
    formatSharePercent(property.ownershipShare)
  )
  const [boughtOnText, setBoughtOnText] = useState<string | null>(null)
  const [errors, setErrors] = useState<PropertyFieldErrors>({})
  const [formError, setFormError] = useState<string | null>(null)

  const boughtOn =
    boughtOnText ??
    (purchaseDate === null ? "" : formatEditorDate(purchaseDate))
  const parsedBoughtOn = useMemo(
    () => parseEditorDate(boughtOn, new Date()),
    [boughtOn]
  )
  const pending = update.isPending || setPurchaseDate.isPending

  function submit() {
    const trimmed = name.trim()
    const share = parseSharePercent(sharePercent)
    const boughtOnIssue =
      purchaseDate === null
        ? null
        : parsedBoughtOn.date === null
          ? DATE_UNREADABLE
          : isAfterNow(parsedBoughtOn.date, new Date())
            ? DATE_IN_FUTURE
            : null
    const next: PropertyFieldErrors = {
      ...(trimmed === "" ? { name: NAME_REQUIRED } : {}),
      ...(share.error === null ? {} : { ownership_share: share.error }),
      ...(boughtOnIssue === null ? {} : { bought_on: boughtOnIssue }),
    }
    setErrors(next)
    setFormError(null)
    if (trimmed === "" || share.percent === null || boughtOnIssue !== null) {
      return
    }

    const movedTo =
      purchaseDate !== null &&
      parsedBoughtOn.date !== null &&
      !isSameDay(parsedBoughtOn.date * 1000, purchaseDate * 1000)
        ? parsedBoughtOn.date
        : null
    const saved = () => {
      onOpenChange(false)
      toast.add({
        type: "success",
        title: "Property saved",
        description: `${trimmed} is up to date.`,
      })
    }

    update.mutate(
      {
        propertyId: property.propertyId,
        body: {
          name: trimmed,
          ownership_share: sharePercentToFraction(share.percent),
        },
      },
      {
        onSuccess: () => {
          if (movedTo === null) {
            saved()
            return
          }
          setPurchaseDate.mutate(
            {
              accountId: property.accountId,
              assetId: property.assetId,
              currencyAssetId: property.currencyAssetId,
              date: movedTo,
            },
            {
              onSuccess: saved,
              onError: (error) => {
                setFormError(error.message)
              },
            }
          )
        },
        onError: (error) => {
          const mapped = propertyServerErrors(error)
          setErrors(mapped.fieldErrors)
          setFormError(mapped.formError)
        },
      }
    )
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit {property.name}</DialogTitle>
          <DialogDescription>
            The name is shared with its account, so both change together.
          </DialogDescription>
        </DialogHeader>
        <form
          id="property-edit-form"
          noValidate
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          {formError === null ? null : (
            <p
              role="alert"
              className="text-[11.5px] leading-[1.5] text-pretty text-negative"
            >
              {formError}
            </p>
          )}
          <Field data-invalid={errors.name !== undefined}>
            <FieldLabel htmlFor="property-edit-name">Name</FieldLabel>
            <Input
              id="property-edit-name"
              value={name}
              autoComplete="off"
              onChange={(event) => {
                setName(event.target.value)
              }}
            />
            <FieldDescription>
              It cannot match another account&rsquo;s name.
            </FieldDescription>
            {errors.name ? <FieldError>{errors.name}</FieldError> : null}
          </Field>
          <ShareField
            id="property-edit-share"
            value={sharePercent}
            error={errors.ownership_share}
            onChange={setSharePercent}
          />
          <Field data-invalid={errors.bought_on !== undefined}>
            <FieldLabel htmlFor="property-edit-bought-on">Bought on</FieldLabel>
            <Input
              id="property-edit-bought-on"
              value={boughtOn}
              autoComplete="off"
              disabled={purchaseDate === null}
              onChange={(event) => {
                setBoughtOnText(event.target.value)
              }}
            />
            <FieldDescription>
              {purchase.isPending
                ? "Looking up when you bought it…"
                : purchaseDate === null
                  ? "Its purchase transaction could not be found."
                  : boughtOnText === null
                    ? BOUGHT_ON_HINT
                    : (parsedBoughtOn.label ?? BOUGHT_ON_HINT)}
            </FieldDescription>
            {errors.bought_on ? (
              <FieldError>{errors.bought_on}</FieldError>
            ) : null}
          </Field>
        </form>
        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            Cancel
          </Button>
          <Button type="submit" form="property-edit-form" disabled={pending}>
            {pending ? "Saving…" : "Save changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
