import SvertoCore
import SwiftUI

struct PossibleDuplicateCard: View {
    let row: ImportStagedRow
    let progress: String?
    let selectedId: String?
    let enabled: Bool
    let onSelect: (String) -> Void
    let onConfirm: () -> Void

    private var options: [QuestionOption] {
        row.nearMiss.map { candidate in
            QuestionOption(
                id: candidate.id,
                title: "\(candidate.description ?? "No description") · \(ImportDates.date(candidate.date))",
                subtitle: [candidate.amountDisplay, candidate.accountName].compactMap { $0 }.joined(separator: " · "),
                note: Self.note(candidate)
            )
        } + [QuestionOption(id: ImportSessionViewModel.newTransactionChoice, title: "None of these — it's new")]
    }

    var body: some View {
        DuplicateQuestionCard(
            progress: progress,
            description: row.description,
            detail: [ImportDates.date(row.date), row.accountName].filter { !$0.isEmpty }.joined(separator: " · "),
            amount: row.amountDisplay,
            enabled: enabled
        ) {
            MatchesLabel(row.nearMiss.count == 1 ? "Possible match in your ledger" : "Possible matches in your ledger")
            OptionList(options: options, selectedId: selectedId, enabled: enabled, onSelect: onSelect)
        } actions: {
            Button("Confirm", action: onConfirm)
                .buttonStyle(.borderedProminent)
                .disabled(selectedId == nil)
                .accessibilityLabel("Confirm the choice for \(row.description), \(row.amountDisplay)")
        }
    }

    private static func note(_ candidate: ImportCandidate) -> String? {
        if !candidate.parts.isEmpty {
            let parts = candidate.parts
                .map { part in [ImportDates.shortDate(part.date), part.description, part.amountDisplay].compactMap { $0 }.joined(separator: " ") }
                .joined(separator: "; ")
            return "\(candidate.parts.count) transactions in your ledger add up to it: \(parts)"
        }
        if !candidate.inLedger { return "From another file in this import" }
        if candidate.leavesMovementUnrecorded { return "Covers only part of this transaction" }
        if candidate.amend { return "Takes the statement's amount" }
        return nil
    }
}

struct DuplicateQuestionCard<Matches: View, Actions: View>: View {
    var supporting: String? = nil
    var progress: String? = nil
    let description: String
    let detail: String
    let amount: String
    let enabled: Bool
    @ViewBuilder let matches: Matches
    @ViewBuilder let actions: Actions

    var body: some View {
        QuestionCard {
            QuestionTitle(title: "Is this already recorded?", supporting: supporting, trailing: progress)
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(description)
                        .lineLimit(2)
                    Text(detail)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                Text(amount)
                    .monospacedDigit()
            }
            .padding(12)
            .background(Color(.tertiarySystemFill), in: .rect(cornerRadius: 12))
            .accessibilityElement(children: .combine)
            .padding(.horizontal, Layout.cardPadding)
            .padding(.bottom, 12)
            matches
            CardActions { actions }
                .disabled(!enabled)
                .padding(.horizontal, Layout.cardPadding)
                .padding(.top, 12)
                .padding(.bottom, 8)
        }
    }
}

struct MatchesLabel: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.footnote.weight(.semibold))
            .foregroundStyle(.secondary)
            .padding(.horizontal, Layout.cardPadding)
            .padding(.bottom, 6)
    }
}

struct StartingBalanceCard: View {
    let row: ImportStagedRow
    let enabled: Bool
    let onAdd: () -> Void
    let onLeaveOut: () -> Void

    private var account: String { row.accountName.isEmpty ? "this account" : row.accountName }
    private var subject: String { "the starting balance of \(account) on \(ImportDates.date(row.date)), \(row.amountDisplay)" }

    var body: some View {
        ReviewCard {
            Text("Start \(account) on \(ImportDates.date(row.date)) at \(row.amountDisplay)?")
                .font(.headline)
                .fixedSize(horizontal: false, vertical: true)
            DetailLine("The statement's first balance needs this before its first row, on top of what your ledger already holds. Add it, leave it out, or add the statement before this one.")
            CardActions {
                Button("Leave out", action: onLeaveOut)
                    .buttonStyle(.bordered)
                    .accessibilityLabel("Leave out \(subject)")
                Button("Add", action: onAdd)
                    .buttonStyle(.borderedProminent)
                    .accessibilityLabel("Add \(subject)")
            }
            .disabled(!enabled)
        }
    }
}
