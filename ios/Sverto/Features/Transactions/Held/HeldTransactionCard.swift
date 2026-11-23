import SvertoCore
import SwiftUI

struct HeldTransactionCard: View {
    let held: HeldTransaction
    let deciding: Bool
    let enabled: Bool
    let onDecide: (HeldTransactionDecision) -> Void
    @State private var selectedId: String?

    private var candidateId: String? {
        held.candidates.first { $0.id == selectedId }?.id ?? held.candidates.first?.id
    }

    private var subject: String { "\(held.description), \(held.amountDisplay)" }

    private var options: [QuestionOption] {
        held.candidates.map { candidate in
            QuestionOption(
                id: candidate.id,
                title: "\(candidate.description ?? "No description") · \(ImportDates.date(candidate.date))",
                subtitle: [candidate.amountDisplay, candidate.categoryName].compactMap { $0 }.joined(separator: " · ")
            )
        }
    }

    var body: some View {
        DuplicateQuestionCard(
            supporting: "From \(ConnectorNames.providerName(held.providerKind))",
            description: held.description,
            detail: [ImportDates.date(held.date), held.accountName].filter { !$0.isEmpty }.joined(separator: " · "),
            amount: held.amountDisplay,
            enabled: enabled
        ) {
            if held.candidates.isEmpty {
                DetailLine("The transaction it looked like was deleted or already matched another payment from this bank account.")
                    .padding(.horizontal, Layout.cardPadding)
            } else {
                MatchesLabel(held.candidates.count == 1 ? "Possible match in your ledger" : "Possible matches in your ledger")
                OptionList(options: options, selectedId: candidateId, enabled: enabled) { selectedId = $0 }
            }
        } actions: {
            if deciding {
                ProgressView()
                    .frame(minHeight: 34)
                    .accessibilityLabel("Saving your choice")
            } else if let candidateId {
                Button("Different — add it") { onDecide(.different) }
                    .buttonStyle(.bordered)
                    .accessibilityLabel("Different, add it: \(subject)")
                Button("Same transaction") { onDecide(.same(transactionId: candidateId)) }
                    .buttonStyle(.borderedProminent)
                    .accessibilityLabel("Same transaction: \(subject)")
            } else {
                Button("Add it") { onDecide(.different) }
                    .buttonStyle(.borderedProminent)
                    .accessibilityLabel("Add it: \(subject)")
            }
        }
    }
}
