import SvertoCore
import SwiftUI

extension ImportBalanceCheck {
    var title: String {
        [accountName, ticker].filter { !$0.isEmpty }.joined(separator: " · ")
    }

    var coverage: String {
        "\(ImportCopy.countOf(balancesChecked, "balance")) checked · \(ImportDates.range(firstDate, lastDate))"
    }
}

struct BalancedAccountsCard: View {
    let checks: [ImportBalanceCheck]

    var body: some View {
        GroupedCard {
            VStack(spacing: 0) {
                ForEach(Array(checks.enumerated()), id: \.element) { index, check in
                    if index > 0 {
                        RowDivider()
                    }
                    LabelWithChip {
                        label(check)
                    } chip: {
                        ToneChip(label: "Balanced", tone: .positive, systemImage: "checkmark")
                    }
                    .padding(.horizontal, Layout.cardPadding)
                    .padding(.vertical, 12)
                    .accessibilityElement(children: .combine)
                }
            }
        }
    }

    private func label(_ check: ImportBalanceCheck) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(check.title)
            Text(check.coverage)
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
    }
}

struct BalanceCheckCard: View {
    let block: BalanceCheckBlock
    let enabled: Bool
    let onEnterBalance: (_ askKey: String, _ quantity: String) -> Void
    let onReviewGaps: () -> Void

    @State private var changingEntered = false

    private var check: ImportBalanceCheck { block.check }
    private var added: Int { block.adjustments.filter { $0.verb == .add }.count }

    var body: some View {
        ReviewCard {
            LabelWithChip {
                heading
            } chip: {
                chip
            }
            if let summary {
                DetailLine(summary)
            }
            if let ask = block.question {
                Text("What was the balance on \(ImportDates.date(ask.asOfDate))?")
                BalanceEntryField(suffix: ask.ticker, enabled: enabled) { onEnterBalance(ask.askKey, $0) }
            }
            if let entered = block.entered {
                enteredBalance(entered)
            }
            if !check.changes.isEmpty || !check.unlisted.isEmpty || added > 0 {
                Button(action: onReviewGaps) {
                    HStack {
                        Text(check.changes.isEmpty ? "Review details" : "Review \(ImportCopy.countOf(check.changes.count, "gap"))")
                            .frame(maxWidth: .infinity, alignment: .leading)
                        Image(systemName: "chevron.forward")
                            .font(.footnote.weight(.semibold))
                            .accessibilityHidden(true)
                    }
                    .frame(minHeight: 44)
                    .contentShape(.rect)
                }
                .buttonStyle(.borderless)
                .accessibilityLabel("Review the gaps in \(check.title)")
            }
        }
    }

    private var heading: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(check.title)
                .font(.headline)
            Text(check.coverage)
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private var chip: some View {
        if check.finalOffBy != 0 {
            ToneChip(label: "Off by \(check.finalOffByDisplay)", tone: .negative)
        } else if check.startingOffBy == 0, check.changes.isEmpty {
            ToneChip(label: "Balanced", tone: .positive, systemImage: "checkmark")
        } else {
            ToneChip(label: "Balanced at the end", tone: .ghost)
        }
    }

    private var summary: String? {
        let notListed = check.unlisted.filter { !$0.mayBookLater }.count
        let deleting = check.unlisted.filter(\.deleting).count
        let parts = [
            check.changes.isEmpty
                ? nil
                : "\(ImportCopy.countOf(check.changes.count, "gap")) between statement balances that nothing in your ledger explains.",
            check.startingOffBy == 0 ? nil : "Off by \(check.startingOffByDisplay) before the first balance, from history before this statement.",
            notListed > 0
                ? "\(ImportCopy.countOf(notListed, "transaction")) in your ledger \(notListed == 1 ? "isn't" : "aren't") on this statement."
                : nil,
            deleting > 0 ? "\(deleting) \(deleting == 1 ? "is" : "are") deleted when you apply." : nil,
            added > 0 ? "Includes \(ImportCopy.countOf(added, "adjustment")) you added." : nil,
        ].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " ")
    }

    @ViewBuilder
    private func enteredBalance(_ entered: ImportEnteredBalance) -> some View {
        if changingEntered {
            Text("What was the balance on \(ImportDates.date(entered.asOfDate))?")
            BalanceEntryField(suffix: check.ticker, enabled: enabled) { quantity in
                changingEntered = false
                onEnterBalance(entered.askKey, quantity)
            }
        } else {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                DetailLine("Checked against the balance you entered: \(entered.quantityDisplay) at the end of \(ImportDates.date(entered.date)).")
                    .frame(maxWidth: .infinity, alignment: .leading)
                Button("Change") { changingEntered = true }
                    .buttonStyle(.borderless)
                    .disabled(!enabled)
                    .accessibilityLabel("Change the balance you entered for \(ImportDates.date(entered.date))")
            }
        }
    }
}

struct UnlistedGroup: View {
    let rows: [ImportUnlistedMovement]
    let enabled: Bool
    let onAnswer: ((_ askKey: String, _ delete: Bool) -> Void)?

    private var groups: [[ImportUnlistedMovement]] {
        var order: [String] = []
        var byKey: [String: [ImportUnlistedMovement]] = [:]
        for row in rows {
            let key = row.askKey ?? row.transactionId
            if byKey[key] == nil { order.append(key) }
            byKey[key, default: []].append(row)
        }
        return order.compactMap { byKey[$0] }
    }

    var body: some View {
        ForEach(groups, id: \.first?.transactionId) { group in
            if let first = group.first {
                HStack(alignment: .center, spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(group, id: \.transactionId) { row in
                            Text([ImportDates.date(row.date), row.description, row.amountDisplay].compactMap { $0 }.joined(separator: " · "))
                                .strikethrough(row.deleting)
                                .foregroundStyle(row.deleting ? Color.loss : Color.primary)
                            if row.explainsDifference {
                                Text("Explains the difference")
                                    .font(.footnote)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        if first.deleting {
                            Text("Deleted when you apply")
                                .font(.footnote)
                                .foregroundStyle(.loss)
                        }
                    }
                    .font(.subheadline)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    if let onAnswer, let askKey = first.askKey {
                        let subject = [first.description, first.amountDisplay, ImportDates.date(first.date)].compactMap { $0 }.joined(separator: ", ")
                        Button(first.deleting ? "Keep" : "Delete on apply") { onAnswer(askKey, !first.deleting) }
                            .buttonStyle(.borderless)
                            .disabled(!enabled)
                            .accessibilityLabel(first.deleting ? "Keep \(subject)" : "Delete \(subject) when you apply")
                    }
                }
            }
        }
    }
}
