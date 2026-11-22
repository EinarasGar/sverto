import type {
  GroupTransactionItem,
  RequiredIdentifiableTransaction,
} from "@/api"
import { accountLabel, readableAssetLabel } from "@/lib/domain/refs"
import { impliedUnitPrice } from "@/lib/domain/transaction-types"
import { formatMoney, formatUnits, pluralise } from "@/lib/format"

import type { LookupIndex } from "./lookup"
import { resolveAccount, resolveAsset } from "./lookup"
import type { LedgerDescription } from "./types"

const ARROW = "→"

function synthesised(
  primary: string,
  detail: string | null
): LedgerDescription {
  return { primary, detail, source: "synthesised" }
}

function quantityAt(
  units: number,
  consideration: number,
  currencyLabel: string
): string {
  const magnitude = Math.abs(units)
  const quantity = `${formatUnits(magnitude)} ${pluralise(magnitude, "unit")}`
  const price = impliedUnitPrice(units, consideration)
  if (price === null) return quantity
  return `${quantity} @ ${formatMoney(price, { currency: currencyLabel })}`
}

/**
 * Only `regular` transactions and groups carry a stored description; the other twelve
 * types are described from their own entries here so every screen renders the same
 * sentence.
 */
export function describeTransaction(
  transaction: RequiredIdentifiableTransaction,
  lookup: LookupIndex
): LedgerDescription {
  switch (transaction.type) {
    case "regular": {
      const stored = transaction.description?.trim() ?? ""
      if (stored !== "") return { primary: stored, detail: null, source: "api" }
      const category = lookup.categories.get(transaction.category_id)
      return synthesised(category?.name ?? "Transaction", null)
    }
    case "asset_purchase": {
      const asset = resolveAsset(lookup, transaction.purchase_change.asset_id)
      const cash = resolveAsset(
        lookup,
        transaction.cash_outgoings_change.asset_id
      )
      return synthesised(
        `Buy ${readableAssetLabel(asset)}`,
        quantityAt(
          transaction.purchase_change.amount,
          transaction.cash_outgoings_change.amount,
          readableAssetLabel(cash)
        )
      )
    }
    case "asset_sale": {
      const asset = resolveAsset(lookup, transaction.sale_entry.asset_id)
      const cash = resolveAsset(lookup, transaction.proceeds_entry.asset_id)
      return synthesised(
        `Sell ${readableAssetLabel(asset)}`,
        quantityAt(
          transaction.sale_entry.amount,
          transaction.proceeds_entry.amount,
          readableAssetLabel(cash)
        )
      )
    }
    case "asset_trade": {
      const outgoing = resolveAsset(lookup, transaction.outgoing_entry.asset_id)
      const incoming = resolveAsset(lookup, transaction.incoming_entry.asset_id)
      return synthesised(
        `Trade ${readableAssetLabel(outgoing)} ${ARROW} ${readableAssetLabel(incoming)}`,
        `${formatUnits(Math.abs(transaction.outgoing_entry.amount), {
          ticker: readableAssetLabel(outgoing),
        })} ${ARROW} ${formatUnits(
          Math.abs(transaction.incoming_entry.amount),
          {
            ticker: readableAssetLabel(incoming),
          }
        )}`
      )
    }
    case "asset_balance_transfer":
    case "cash_balance_transfer": {
      const asset = resolveAsset(lookup, transaction.outgoing_change.asset_id)
      const from = resolveAccount(
        lookup,
        transaction.outgoing_change.account_id
      )
      const to = resolveAccount(lookup, transaction.incoming_change.account_id)
      return synthesised(
        `Move ${readableAssetLabel(asset)}`,
        `${accountLabel(from)} ${ARROW} ${accountLabel(to)}`
      )
    }
    case "cash_dividend": {
      const origin = resolveAsset(lookup, transaction.origin_asset_id)
      return synthesised(`Dividend from ${readableAssetLabel(origin)}`, null)
    }
    case "asset_dividend": {
      const asset = resolveAsset(lookup, transaction.entry.asset_id)
      return synthesised(`Dividend in ${readableAssetLabel(asset)}`, null)
    }
    case "asset_transfer_in": {
      const asset = resolveAsset(lookup, transaction.entry.asset_id)
      return synthesised(`Transfer in ${readableAssetLabel(asset)}`, null)
    }
    case "asset_transfer_out": {
      const asset = resolveAsset(lookup, transaction.entry.asset_id)
      return synthesised(`Transfer out ${readableAssetLabel(asset)}`, null)
    }
    case "cash_transfer_in":
      return synthesised("Money in", null)
    case "cash_transfer_out":
      return synthesised("Money out", null)
    case "account_fees":
      return synthesised("Account fee", null)
  }
}

export function describeGroup(
  group: GroupTransactionItem,
  lookup: LookupIndex
): LedgerDescription {
  const stored = group.description.trim()
  if (stored !== "") return { primary: stored, detail: null, source: "api" }
  const category = lookup.categories.get(group.category_id)
  return synthesised(category?.name ?? "Group", null)
}
