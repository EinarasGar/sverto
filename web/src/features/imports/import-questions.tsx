import { useState } from "react"

import type {
  AccountMappingAsk,
  AssetMappingAsk,
  IdentifiableImportAsk,
  ImportAnswer,
} from "@/api"
import {
  EntityPicker,
  Panel,
  PanelHeader,
  PanelTitle,
  type PickerOption,
} from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { AccountEditor, useAccountEditor } from "@/features/accounts"
import { useAccounts } from "@/features/accounts/api"
import { useAssetSearch } from "@/features/assets/api"
import { countOf } from "@/lib/format"

import type { ImportFileView, ImportSessionParams } from "./api"
import { useAnswerAsks, useImportPending } from "./api"
import {
  accountPrefill,
  leaveOutConsequence,
  questionAnchor,
} from "./link-copy"

function accountAnswer(accountId: string | null): ImportAnswer {
  const answer =
    accountId === null
      ? { kind: "account_mapping", choice: "exclude" }
      : { kind: "account_mapping", choice: "map", account_id: accountId }
  return answer as ImportAnswer
}

function assetAnswer(assetId: number | null): ImportAnswer {
  const answer =
    assetId === null
      ? { kind: "asset_mapping", choice: "exclude" }
      : { kind: "asset_mapping", choice: "map", asset_id: assetId }
  return answer as ImportAnswer
}

function AccountQuestion({
  params,
  name,
  asks,
  fileNames,
}: {
  params: ImportSessionParams
  name: string
  asks: readonly AccountMappingAsk[]
  fileNames: Readonly<Record<string, string>>
}) {
  const accounts = useAccounts(params.userId)
  const answer = useAnswerAsks(params)
  const editor = useAccountEditor()
  const [split, setSplit] = useState(false)
  const [chosen, setChosen] = useState<Record<string, string | null>>({})
  const [creatingFor, setCreatingFor] = useState<readonly AccountMappingAsk[]>(
    []
  )

  const suggested = new Set(
    asks.flatMap((ask) => ask.candidates.map((candidate) => candidate.id))
  )
  const options: PickerOption[] = (accounts.data?.accounts ?? [])
    .map((account) => ({
      value: account.accountId,
      label: account.name,
      group: suggested.has(account.accountId) ? "Suggested" : "Your accounts",
    }))
    .sort((a, b) =>
      a.group === b.group ? 0 : a.group === "Suggested" ? -1 : 1
    )

  function answerAll(
    targets: readonly AccountMappingAsk[],
    accountId: string | null
  ) {
    answer.mutate({
      answers: targets.map((ask) => ({
        askKey: ask.ask_key,
        answer: accountAnswer(accountId),
      })),
    })
  }

  const splittable =
    asks.length > 1 && !asks.some((ask) => ask.answer_covers_all_files)
  const groups = split && splittable ? asks.map((ask) => [ask]) : [asks]

  return (
    <li
      id={questionAnchor("account", name)}
      className="flex flex-col gap-3 px-[18px] py-4"
    >
      <div>
        <p className="text-[12.5px] font-medium text-ink">
          Which account is “{name}”?
        </p>
        <p className="mt-1 text-[11px] text-ink-3">
          In {asks.map((ask) => fileNames[ask.file_id] ?? "a file").join(", ")}
        </p>
      </div>
      {groups.map((group) => {
        const key = group.map((ask) => ask.ask_key).join("|")
        const value = Object.hasOwn(chosen, key)
          ? (chosen[key] ?? null)
          : (group[0]?.candidates[0]?.id ?? null)
        return (
          <div key={key} className="flex flex-wrap items-center gap-2">
            {split ? (
              <span className="w-full text-[11px] text-ink-2">
                {fileNames[group[0]?.file_id ?? ""] ?? "File"}
              </span>
            ) : null}
            <EntityPicker
              size="sm"
              className="sm:w-[280px]"
              value={value}
              onValueChange={(next) => {
                setChosen((previous) => ({ ...previous, [key]: next }))
              }}
              options={options}
              placeholder="Pick an account"
            />
            <Button
              size="sm"
              disabled={value === null || answer.isPending}
              onClick={() => {
                answerAll(group, value)
              }}
            >
              Link
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={answer.isPending}
              onClick={() => {
                setCreatingFor(group)
                editor.openCreate(accountPrefill(name))
              }}
            >
              Create account
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={answer.isPending}
              onClick={() => {
                answerAll(group, null)
              }}
            >
              Don’t import
            </Button>
          </div>
        )
      })}
      {splittable ? (
        <button
          type="button"
          className="self-start text-[11px] text-ink-3 underline-offset-2 hover:text-ink-2 hover:underline"
          onClick={() => {
            setSplit((previous) => !previous)
          }}
        >
          {split
            ? "These are the same account"
            : "These are different accounts"}
        </button>
      ) : null}
      <AccountEditor
        userId={params.userId}
        mode={editor.mode}
        open={editor.open}
        onOpenChange={editor.setOpen}
        onCreated={(account) => {
          answerAll(creatingFor, account.accountId)
        }}
      />
    </li>
  )
}

function AssetQuestion({
  params,
  ask,
  pending,
}: {
  params: ImportSessionParams
  ask: AssetMappingAsk
  pending: number
}) {
  const answer = useAnswerAsks(params)
  const search = useAssetSearch()
  const waiting = useImportPending(params, pending > 0)
  const [leavingOut, setLeavingOut] = useState(false)
  const [value, setValue] = useState<string | null>(
    ask.candidates[0] === undefined ? null : String(ask.candidates[0].id)
  )

  const candidateIds = new Set(ask.candidates.map((c) => String(c.id)))
  const options: PickerOption[] = [
    ...ask.candidates.map((candidate) => ({
      value: String(candidate.id),
      label: candidate.ticker,
      subLabel: candidate.name,
      group: "Suggested",
    })),
    ...search.assets
      .filter((asset) => !candidateIds.has(String(asset.assetId)))
      .map((asset) => ({
        value: String(asset.assetId),
        label: asset.ticker ?? String(asset.assetId),
        ...(asset.name === null ? {} : { subLabel: asset.name }),
        group: "Search results",
      })),
  ]

  return (
    <li
      id={questionAnchor("asset", ask.raw)}
      className="flex flex-col gap-3 px-[18px] py-4"
    >
      <p className="text-[12.5px] font-medium text-ink">
        Which asset is “{ask.raw}”?
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <EntityPicker
          size="sm"
          className="sm:w-[280px]"
          value={value}
          onValueChange={setValue}
          options={options}
          placeholder="Search assets"
          search={{
            query: search.query,
            onQueryChange: search.setQuery,
            pending: search.pending,
            hasMore: search.hasMore,
            onLoadMore: search.loadMore,
            total: search.total,
          }}
        />
        <Button
          size="sm"
          disabled={value === null || answer.isPending}
          onClick={() => {
            if (value === null) return
            answer.mutate({
              answers: [
                { askKey: ask.ask_key, answer: assetAnswer(Number(value)) },
              ],
            })
          }}
        >
          Link
        </Button>
        {ask.choices.includes("exclude") && !leavingOut ? (
          <Button
            size="sm"
            variant="ghost"
            disabled={answer.isPending}
            onClick={() => {
              setLeavingOut(true)
            }}
          >
            Leave this asset out
          </Button>
        ) : null}
      </div>
      {leavingOut ? (
        <div
          role="group"
          aria-label={`Leave “${ask.raw}” out`}
          className="flex flex-col gap-2"
        >
          <p className="text-[11px] leading-[1.5] text-pretty text-ink-2">
            {pending > 0 && waiting.data === undefined
              ? "Counting the rows this affects…"
              : leaveOutConsequence(ask.raw, waiting.data ?? [])}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              size="sm"
              variant="destructive"
              disabled={
                answer.isPending || (pending > 0 && waiting.data === undefined)
              }
              onClick={() => {
                answer.mutate({
                  answers: [{ askKey: ask.ask_key, answer: assetAnswer(null) }],
                })
              }}
            >
              Leave it out
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={answer.isPending}
              onClick={() => {
                setLeavingOut(false)
              }}
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : null}
    </li>
  )
}

export function ImportQuestions({
  params,
  asks,
  views,
  pending,
}: {
  params: ImportSessionParams
  asks: readonly IdentifiableImportAsk[]
  views: readonly ImportFileView[]
  pending: number
}) {
  const fileNames: Record<string, string> = {}
  for (const view of views) {
    if (view.details !== undefined) {
      fileNames[view.file.file_id] = view.details.original_name
    }
  }
  const accountsByName = new Map<string, AccountMappingAsk[]>()
  const assets: AssetMappingAsk[] = []
  for (const ask of asks) {
    if (ask.kind === "account_mapping") {
      accountsByName.set(ask.raw_account, [
        ...(accountsByName.get(ask.raw_account) ?? []),
        ask,
      ])
    } else if (ask.kind === "asset_mapping") {
      assets.push(ask)
    }
  }
  const count = accountsByName.size + assets.length
  if (count === 0) return null

  return (
    <Panel data-slot="import-questions">
      <PanelHeader>
        <PanelTitle>Questions</PanelTitle>
        <span className="text-[11px] leading-none font-medium text-ink-3">
          {countOf(count, "question")}
        </span>
      </PanelHeader>
      <ul
        aria-label="Questions about this import"
        className="divide-y divide-border"
      >
        {[...accountsByName.entries()].map(([name, group]) => (
          <AccountQuestion
            key={name}
            params={params}
            name={name}
            asks={group}
            fileNames={fileNames}
          />
        ))}
        {assets.map((ask) => (
          <AssetQuestion
            key={ask.ask_key}
            params={params}
            ask={ask}
            pending={pending}
          />
        ))}
      </ul>
    </Panel>
  )
}
