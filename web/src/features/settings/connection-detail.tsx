import { useState } from "react"
import { Link, useNavigate } from "@tanstack/react-router"

import type { ConnectorImportFrom } from "@/api"
import { useUserId } from "@/auth"
import { countOf, formatDateStamp, formatMoney } from "@/lib/format"
import type { AccountId, ConnectionId } from "@/lib/query"
import { PanelBoundary } from "@/components/layout/route-boundaries"
import {
  EntityPicker,
  SegmentedControl,
  type SegmentedOption,
  StatusChip,
} from "@/components/primitives"
import { ErrorState } from "@/components/states/message-state"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import type { AccountConnector } from "@/features/accounts/api"
import { useAccountsSuspense } from "@/features/accounts/api"

import type {
  ConnectionSummary,
  CreateBindingsVariables,
  ProviderAccountRef,
} from "./api"
import {
  useConnectionsSuspense,
  useCreateBindings,
  useCreateConnectorImport,
  useDeleteBinding,
  useProviderAccountsSuspense,
  useRevokeConnection,
  useSyncBinding,
  useUpdateBinding,
} from "./api"
import {
  SettingsBindingRow,
  SettingsBindings,
  SettingsBindingTarget,
  SettingsBlock,
  SettingsBlocks,
  SettingsConsequence,
  SettingsDanger,
  SettingsList,
  SettingsListRow,
} from "./blocks"
import { ConfirmDestructive } from "./confirm-dialog"
import {
  BINDING_UNBOUND_CONSEQUENCE,
  CONNECTION_IMPORT_TOTAL_UNAVAILABLE,
  CONSENT_CONSEQUENCE,
  HISTORY_FAILED,
  HISTORY_FETCHING,
  IMPORT_ACCOUNTS_CONSEQUENCE,
  IMPORT_FROM_AFTER_LAST_ENTRY,
  IMPORT_FROM_ALL,
  IMPORT_FROM_DATE,
  LINK_ACCOUNTS_CONSEQUENCE,
  PAUSE_CONSEQUENCE,
  PENDING_BINDING_IMPORTING,
  REMOVE_BINDING_BODY,
  REMOVE_BINDING_TITLE,
  RESUME_CONSEQUENCE,
  REVOKE_CONNECTION_BODY,
  REVOKE_CONNECTION_TITLE,
  TRUSTED_WRITES_OFF,
  TRUSTED_WRITES_ON,
} from "./copy"
import { consentLabel, syncedLabel } from "./presentation"
import { SettingsListSkeleton } from "./skeletons"

type ImportFromKind = ConnectorImportFrom["kind"]

const IMPORT_FROM_OPTIONS: readonly SegmentedOption<ImportFromKind>[] = [
  { value: "after_last_entry", label: "After my last entry" },
  { value: "all", label: "All available" },
  { value: "date", label: "From a date" },
]

const IMPORT_FROM_CONSEQUENCE: Record<ImportFromKind, string> = {
  after_last_entry: IMPORT_FROM_AFTER_LAST_ENTRY,
  all: IMPORT_FROM_ALL,
  date: IMPORT_FROM_DATE,
}

function providerMetaOf(account: ProviderAccountRef): string {
  const parts = [
    account.currency,
    account.accountNumberLast4 && `••${account.accountNumberLast4}`,
    account.balance &&
      formatMoney(account.balance.amount, {
        currency: account.balance.currency,
      }),
    account.history === "fetching" && HISTORY_FETCHING,
    account.history === "failed" && HISTORY_FAILED,
  ].filter((part): part is string => typeof part === "string" && part !== "")
  return parts.length === 0 ? account.providerAccountId : parts.join(" · ")
}

function PendingBindingImport({ binding }: { binding: AccountConnector }) {
  return (
    <>
      <div className="min-w-[min(100%,220px)] flex-1">
        <p className="text-[11.5px] leading-[1.3] font-semibold">
          Being imported
        </p>
        <p className="text-[11px] leading-[1.5] text-pretty text-ink-3">
          {PENDING_BINDING_IMPORTING}
        </p>
      </div>
      {binding.importSessionId === null ? null : (
        <Button
          variant="ghost"
          size="xs"
          render={
            <Link
              to="/imports/$sessionId"
              params={{ sessionId: binding.importSessionId }}
            />
          }
        >
          Open the import
        </Button>
      )}
    </>
  )
}

function BindingActions({
  binding,
  onRemove,
}: {
  binding: AccountConnector
  onRemove: () => void
}) {
  const userId = useUserId()
  const update = useUpdateBinding(userId)
  const sync = useSyncBinding(userId)
  const paused = binding.status === "paused"

  return (
    <>
      <Switch
        checked={binding.writesPostDirectly}
        disabled={update.isPending}
        aria-label="Trusted writes"
        onCheckedChange={(checked) => {
          update.mutate({
            bindingId: binding.bindingId,
            paused,
            writesPostDirectly: checked,
          })
        }}
      />
      <div className="min-w-[min(100%,220px)] flex-1">
        <p className="text-[11.5px] leading-[1.3] font-semibold">
          Trusted writes
        </p>
        <p className="text-[11px] leading-[1.5] text-pretty text-ink-3">
          {binding.writesPostDirectly ? TRUSTED_WRITES_ON : TRUSTED_WRITES_OFF}
        </p>
        <p className="text-[11px] leading-[1.5] text-pretty text-ink-3">
          {paused ? RESUME_CONSEQUENCE : PAUSE_CONSEQUENCE}
        </p>
      </div>
      <Button
        variant="ghost"
        size="xs"
        disabled={sync.isPending}
        onClick={() => {
          sync.mutate({ bindingId: binding.bindingId })
        }}
      >
        {sync.isPending ? "Syncing…" : "Sync now"}
      </Button>
      <Button
        variant="ghost"
        size="xs"
        disabled={update.isPending}
        onClick={() => {
          update.mutate({
            bindingId: binding.bindingId,
            paused: !paused,
            writesPostDirectly: binding.writesPostDirectly,
          })
        }}
      >
        {paused ? "Resume" : "Pause"}
      </Button>
      <Button
        variant="ghost"
        size="xs"
        className="text-negative hover:text-negative"
        onClick={onRemove}
      >
        Remove
      </Button>
    </>
  )
}

function AccountPicker({
  value,
  onValueChange,
  placeholder,
}: {
  value: AccountId | null
  onValueChange: (next: AccountId | null) => void
  placeholder: string
}) {
  const accounts = useAccountsSuspense(useUserId())

  return (
    <EntityPicker
      size="sm"
      label="Pick an account"
      placeholder={placeholder}
      className="w-[190px]"
      value={value}
      options={accounts.accounts.map((account) => ({
        value: account.accountId,
        label: account.name,
        ...(account.accountTypeName === null
          ? {}
          : { subLabel: account.accountTypeName }),
      }))}
      onValueChange={onValueChange}
    />
  )
}

function LinkAccountsRow({
  connectionId,
  accounts,
  linksByImport,
}: {
  connectionId: ConnectionId
  accounts: CreateBindingsVariables["accounts"]
  linksByImport: boolean
}) {
  const userId = useUserId()
  const createImport = useCreateConnectorImport(userId)
  const createBindings = useCreateBindings(userId)
  const navigate = useNavigate()
  const [kind, setKind] = useState<ImportFromKind>("after_last_entry")
  const [date, setDate] = useState("")
  const ready = accounts.length > 0 && (kind !== "date" || date !== "")
  const pending = createImport.isPending || createBindings.isPending

  return (
    <SettingsListRow
      label="Import transactions from"
      consequence={`${IMPORT_FROM_CONSEQUENCE[kind]} ${linksByImport ? IMPORT_ACCOUNTS_CONSEQUENCE : LINK_ACCOUNTS_CONSEQUENCE}`}
      control={
        <SegmentedControl
          label="Import transactions from"
          value={kind}
          onValueChange={setKind}
          options={IMPORT_FROM_OPTIONS}
        />
      }
    >
      <div className="mt-3 flex flex-wrap items-center gap-2">
        {kind === "date" ? (
          <Input
            type="date"
            aria-label="Import from date"
            className="h-7 w-[160px] py-0"
            value={date}
            onChange={(event) => {
              setDate(event.target.value)
            }}
          />
        ) : null}
        <Button
          size="sm"
          disabled={!ready || pending}
          onClick={() => {
            const importFrom: ConnectorImportFrom =
              kind === "date" ? { kind, date } : { kind }
            if (!linksByImport) {
              createBindings.mutate({ connectionId, accounts, importFrom })
              return
            }
            createImport.mutate(
              { connectionId, accounts, importFrom },
              {
                onSuccess: (sessionId) => {
                  void navigate({
                    to: "/imports/$sessionId",
                    params: { sessionId },
                  })
                },
              }
            )
          }}
        >
          {pending
            ? linksByImport
              ? "Importing…"
              : "Linking…"
            : `${linksByImport ? "Import" : "Link"} ${countOf(accounts.length, "account")}`}
        </Button>
      </div>
    </SettingsListRow>
  )
}

function BindingsBlock({ connection }: { connection: ConnectionSummary }) {
  const userId = useUserId()
  const providerAccounts = useProviderAccountsSuspense(
    userId,
    connection.connectionId
  )
  const accounts = useAccountsSuspense(userId)
  const remove = useDeleteBinding(userId)
  const [pending, setPending] = useState<AccountConnector | null>(null)
  const [targets, setTargets] = useState<
    Readonly<Record<string, AccountId | null>>
  >({})

  const bindingByProviderAccount = new Map(
    connection.bindings.map((binding) => [binding.providerAccountId, binding])
  )
  const unbound = providerAccounts.filter(
    (account) => !bindingByProviderAccount.has(account.providerAccountId)
  )
  const chosen = unbound.flatMap((account) => {
    const accountId = targets[account.providerAccountId] ?? null
    return accountId === null
      ? []
      : [{ providerAccountId: account.providerAccountId, accountId }]
  })

  return (
    <>
      <SettingsBindings footnote={BINDING_UNBOUND_CONSEQUENCE}>
        {providerAccounts.map((providerAccount) => {
          const binding = bindingByProviderAccount.get(
            providerAccount.providerAccountId
          )
          const bound =
            binding === undefined ? undefined : accounts.byId[binding.accountId]

          return (
            <SettingsBindingRow
              key={providerAccount.providerAccountId}
              provider={providerAccount.displayName}
              providerMeta={providerMetaOf(providerAccount)}
              status={
                <StatusChip
                  size="binding"
                  status={binding?.statusWord ?? "notLinked"}
                />
              }
              meta={
                binding === undefined
                  ? "never synced"
                  : syncedLabel(binding.lastSyncAt)
              }
              target={
                binding === undefined ? (
                  <AccountPicker
                    value={targets[providerAccount.providerAccountId] ?? null}
                    onValueChange={(accountId) => {
                      setTargets((previous) => ({
                        ...previous,
                        [providerAccount.providerAccountId]: accountId,
                      }))
                    }}
                    placeholder="Leave unlinked"
                  />
                ) : (
                  <SettingsBindingTarget
                    title={bound?.name ?? "Account not in your list"}
                    meta={
                      bound === undefined
                        ? "This binding points at an account you can no longer see."
                        : `${bound.accountTypeName ?? "—"} · ${bound.ownershipSharePercent}% yours`
                    }
                  />
                )
              }
            >
              {binding === undefined ? null : binding.status === "pending" ? (
                <PendingBindingImport binding={binding} />
              ) : (
                <BindingActions
                  binding={binding}
                  onRemove={() => {
                    setPending(binding)
                  }}
                />
              )}
            </SettingsBindingRow>
          )
        })}
        {connection.bindings
          .filter(
            (binding) =>
              !providerAccounts.some(
                (account) =>
                  account.providerAccountId === binding.providerAccountId
              )
          )
          .map((binding) => (
            <SettingsBindingRow
              key={binding.bindingId}
              provider="Provider account no longer listed"
              providerMeta={binding.providerAccountId}
              status={<StatusChip size="binding" status={binding.statusWord} />}
              meta={syncedLabel(binding.lastSyncAt)}
              target={
                <SettingsBindingTarget
                  title={
                    accounts.byId[binding.accountId]?.name ??
                    "Account not in your list"
                  }
                  meta="The provider stopped returning this account, so nothing new can arrive for it."
                />
              }
            >
              {binding.status === "pending" ? (
                <PendingBindingImport binding={binding} />
              ) : (
                <BindingActions
                  binding={binding}
                  onRemove={() => {
                    setPending(binding)
                  }}
                />
              )}
            </SettingsBindingRow>
          ))}
        {unbound.length > 0 ? (
          <LinkAccountsRow
            connectionId={connection.connectionId}
            accounts={chosen}
            linksByImport={connection.linksByImport}
          />
        ) : null}
      </SettingsBindings>
      <ConfirmDestructive
        open={pending !== null}
        onOpenChange={(open) => {
          if (!open) setPending(null)
        }}
        title={REMOVE_BINDING_TITLE}
        lost={REMOVE_BINDING_BODY}
        confirmLabel="Unbind"
        pending={remove.isPending}
        onConfirm={() => {
          const target = pending
          if (target === null) return
          remove.mutate(
            { bindingId: target.bindingId },
            {
              onSettled: () => {
                setPending(null)
              },
            }
          )
        }}
      />
    </>
  )
}

function StatusBlock({ connection }: { connection: ConnectionSummary }) {
  return (
    <SettingsList>
      <SettingsListRow
        label="Connection"
        chip={<StatusChip status={connection.statusWord} />}
        consequence={`${syncedLabel(connection.lastSyncAt)}. ${CONNECTION_IMPORT_TOTAL_UNAVAILABLE}`}
        control={
          <span className="text-[11px] leading-none text-ink-3">
            connected{" "}
            {formatDateStamp(connection.createdAt, { year: "always" })}
          </span>
        }
      />
      <SettingsListRow
        label="Consent"
        consequence={CONSENT_CONSEQUENCE}
        control={
          <span className="font-mono text-[12px] leading-none text-ink-2">
            {consentLabel(connection.consentExpiresAt)}
          </span>
        }
      />
    </SettingsList>
  )
}

function DangerBlock({ connection }: { connection: ConnectionSummary }) {
  const userId = useUserId()
  const revoke = useRevokeConnection(userId)
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)

  return (
    <>
      <SettingsDanger
        title={REVOKE_CONNECTION_TITLE}
        lost={REVOKE_CONNECTION_BODY}
        action={
          <Button
            variant="destructive"
            size="lg"
            onClick={() => {
              setOpen(true)
            }}
          >
            Revoke access
          </Button>
        }
      />
      <ConfirmDestructive
        open={open}
        onOpenChange={setOpen}
        title={`Revoke ${connection.providerLabel}?`}
        lost={REVOKE_CONNECTION_BODY}
        confirmLabel="Revoke access"
        pending={revoke.isPending}
        onConfirm={() => {
          revoke.mutate(
            { connectionId: connection.connectionId },
            {
              onSuccess: () => {
                setOpen(false)
                void navigate({
                  to: "/settings",
                  search: { section: "connections" },
                })
              },
            }
          )
        }}
      />
    </>
  )
}

function ConnectionBody({ connectionId }: { connectionId: ConnectionId }) {
  const connections = useConnectionsSuspense(useUserId())
  const connection = connections.byId[connectionId]

  if (connection === undefined) {
    return (
      <ErrorState
        headline="That connection is not on your account"
        body="It may have been revoked from this page or from the provider. The connections list above is the live one."
      />
    )
  }

  return (
    <SettingsBlocks>
      <SettingsBlock title="Status">
        <StatusBlock connection={connection} />
      </SettingsBlock>

      <SettingsBlock
        title="Account bindings"
        note={`${connection.boundCount} bound`}
      >
        <PanelBoundary pending={<SettingsListSkeleton />}>
          <BindingsBlock connection={connection} />
        </PanelBoundary>
        <SettingsConsequence>
          A provider account only imports once it is bound. Binding is
          reversible and never rewrites what has already arrived.
        </SettingsConsequence>
      </SettingsBlock>

      <SettingsBlock title="Danger zone">
        <DangerBlock connection={connection} />
      </SettingsBlock>
    </SettingsBlocks>
  )
}

export function ConnectionDetail({
  connectionId,
}: {
  connectionId: ConnectionId
}) {
  return (
    <PanelBoundary pending={<SettingsListSkeleton rows={4} />}>
      <ConnectionBody connectionId={connectionId} />
    </PanelBoundary>
  )
}
