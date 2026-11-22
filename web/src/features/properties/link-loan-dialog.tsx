import { useMemo, useState } from "react"

import { useUserId } from "@/auth"
import type { PickerOption } from "@/components/primitives"
import { EntityPicker } from "@/components/primitives"
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
import { toast } from "@/components/ui/toast"
import { useAccountsSuspense } from "@/features/accounts/api"

import type { Property } from "./api"
import { useLinkPropertyLoan, usePropertiesSuspense } from "./api"
import { linkableLoans } from "./loans"

export function LinkLoanDialog({
  property,
  onOpenChange,
}: {
  property: Property
  onOpenChange: (open: boolean) => void
}) {
  const userId = useUserId()
  const accounts = useAccountsSuspense(userId)
  const properties = usePropertiesSuspense(userId)
  const link = useLinkPropertyLoan(userId)
  const [selected, setSelected] = useState<string | null>(null)
  const [issue, setIssue] = useState<string | null>(null)

  const options = useMemo(
    (): PickerOption[] =>
      linkableLoans(accounts, properties).map((account) => ({
        value: account.accountId,
        label: account.name,
        ...(account.accountTypeName === null
          ? {}
          : { subLabel: account.accountTypeName }),
        identity: account.accountId,
      })),
    [accounts, properties]
  )

  function submit() {
    if (selected === null) {
      setIssue("Choose the loan to link.")
      return
    }
    const name = accounts.byId[selected]?.name ?? "The loan"
    link.mutate(
      { propertyId: property.propertyId, accountId: selected },
      {
        onSuccess: () => {
          onOpenChange(false)
          toast.add({
            type: "success",
            title: "Loan linked",
            description: `${name} now counts against ${property.name} and leaves your accounts list.`,
          })
        },
      }
    )
  }

  const conflict =
    link.error?.kind === "conflict"
      ? "That loan is already linked to another property."
      : null

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Link a loan</DialogTitle>
          <DialogDescription>
            A linked loan is subtracted from {property.name} to give its equity,
            and leaves your accounts list. It stays a normal account everywhere
            else.
          </DialogDescription>
        </DialogHeader>
        <Field data-invalid={(issue ?? conflict) !== null}>
          <FieldLabel htmlFor="property-link-loan">Loan</FieldLabel>
          <EntityPicker
            id="property-link-loan"
            value={selected}
            placeholder="Search mortgages and loans"
            emptyLabel="No mortgage or loan account is free to link."
            invalid={(issue ?? conflict) !== null}
            options={options}
            onValueChange={(next) => {
              setIssue(null)
              setSelected(next)
            }}
          />
          <FieldDescription>
            {options.length === 0
              ? "Every mortgage and loan account is already linked, or you have none. Add one as an account first."
              : "Mortgage and loan accounts that are not linked to a property yet."}
          </FieldDescription>
          {(issue ?? conflict) === null ? null : (
            <FieldError>{issue ?? conflict}</FieldError>
          )}
        </Field>
        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            Cancel
          </Button>
          <Button disabled={link.isPending} onClick={submit}>
            {link.isPending ? "Linking…" : "Link loan"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
