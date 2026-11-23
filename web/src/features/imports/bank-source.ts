import type { ImportBankAccountSource } from "@/api"
import { providerName } from "@/features/settings/api"

export interface BankSourceLabel {
  readonly title: string
  readonly meta: string
}

export function bankSourceLabel(
  source: ImportBankAccountSource
): BankSourceLabel {
  const provider = providerName(source.provider_kind)
  const name = source.display_name ?? ""
  const parts = [
    source.currency,
    source.account_number_last4 && `••${source.account_number_last4}`,
    name === "" ? null : provider,
  ].filter((part): part is string => typeof part === "string" && part !== "")
  return {
    title: name === "" ? provider : name,
    meta: parts.join(" · "),
  }
}
