import Foundation

nonisolated enum TypeConversion {
    static func convert(_ state: TransactionFormState, from: TransactionTypeConfig, to: TransactionTypeConfig) -> TransactionFormState {
        guard from.apiType != to.apiType else { return state }

        let sources = materialize(state, mode: from.entryMode)
        let (primarySource, secondarySource) = assignSlots(sources, from: from.entryMode, to: to.entryMode)

        let targetPrimarySign = to.entryMode.primarySign
        let targetSecondarySign: AmountSign = if case let .dual(mode) = to.entryMode { mode.secondarySign } else { .any }

        var primaryEntry = primarySource?.entryFormState(sign: targetPrimarySign) ?? EntryFormState()
        if primaryEntry.amount.isEmpty { primaryEntry.amount = targetPrimarySign.blankAmount }
        let secondaryEntry = secondarySource?.entryFormState(sign: targetSecondarySign) ?? EntryFormState()

        if case let .dual(mode) = to.entryMode, let filled = primarySource ?? secondarySource {
            if mode.sameAccount, primaryEntry.accountId == nil {
                primaryEntry.accountId = filled.accountId
                primaryEntry.accountName = filled.accountName
            }
            if mode.sameAsset, primaryEntry.assetId == nil {
                primaryEntry.assetId = filled.assetId
                primaryEntry.assetDisplay = filled.assetDisplay
            }
            if mode.sharedAmount, primaryEntry.amount.isBlank, let amount = filled.signedAmount {
                primaryEntry.amount = amountText(amount, sign: targetPrimarySign)
            }
        }

        var converted = state
        converted.primaryEntry = primaryEntry
        converted.secondaryEntry = secondaryEntry
        return converted
    }

    private struct SourceEntry {
        let entryId: Int32?
        let accountId: String?
        let accountName: String
        let assetId: Int32?
        let assetDisplay: String
        let signedAmount: Double?

        func matches(_ sign: AmountSign) -> Bool {
            guard let amount = signedAmount, amount != 0 else { return false }
            switch sign {
            case .positive: return amount > 0
            case .negative: return amount < 0
            case .any: return false
            }
        }

        func entryFormState(sign: AmountSign) -> EntryFormState {
            EntryFormState(
                entryId: entryId,
                accountId: accountId,
                accountName: accountName,
                assetId: assetId,
                assetDisplay: assetDisplay,
                amount: signedAmount.map { amountText($0, sign: sign) } ?? ""
            )
        }
    }

    private static func materialize(_ state: TransactionFormState, mode: EntryMode) -> [SourceEntry] {
        switch mode {
        case let .single(single):
            return [source(state.primaryEntry, sign: single.amountSign)]
        case let .dual(dual):
            let primary = source(state.primaryEntry, sign: dual.primarySign)
            let secondary = state.secondaryEntry
            let secondaryAmountRaw = dual.sharedAmount ? state.primaryEntry.amount : secondary.amount
            return [
                primary,
                SourceEntry(
                    entryId: secondary.entryId,
                    accountId: dual.sameAccount ? primary.accountId : secondary.accountId,
                    accountName: dual.sameAccount ? primary.accountName : secondary.accountName,
                    assetId: dual.sameAsset ? primary.assetId : secondary.assetId,
                    assetDisplay: dual.sameAsset ? primary.assetDisplay : secondary.assetDisplay,
                    signedAmount: EditableAmount.signed(secondaryAmountRaw, sign: dual.secondarySign)
                ),
            ]
        }
    }

    private static func source(_ entry: EntryFormState, sign: AmountSign) -> SourceEntry {
        SourceEntry(
            entryId: entry.entryId,
            accountId: entry.accountId,
            accountName: entry.accountName,
            assetId: entry.assetId,
            assetDisplay: entry.assetDisplay,
            signedAmount: EditableAmount.signed(entry.amount, sign: sign)
        )
    }

    private static func assignSlots(_ sources: [SourceEntry], from: EntryMode, to: EntryMode) -> (SourceEntry?, SourceEntry?) {
        switch (from, to) {
        case (.single, .single):
            return (sources[0], nil)
        case let (.dual, .single(target)):
            return (matchSingleTarget(sources, sign: target.amountSign), nil)
        case let (.single, .dual(target)):
            return matchDualTarget(sources[0], target: target)
        case (.dual, .dual):
            return (sources[0], sources[1])
        }
    }

    private static func matchSingleTarget(_ sources: [SourceEntry], sign: AmountSign) -> SourceEntry {
        sources.first { $0.matches(sign) }
            ?? sources.first { $0.accountId != nil }
            ?? sources[0]
    }

    private static func matchDualTarget(_ source: SourceEntry, target: DualEntryMode) -> (SourceEntry?, SourceEntry?) {
        if source.matches(target.primarySign) { return (source, nil) }
        if source.matches(target.secondarySign) { return (nil, source) }
        return (source, nil)
    }

    private static func amountText(_ value: Double, sign: AmountSign) -> String {
        EditableAmount.string(sign == .any ? value : abs(value))
    }
}
