import SwiftUI
import SvertoCore

struct TransactionDaySections: View {
    let groups: [TransactionDayGroup]
    var selectedIds: Set<String> = []
    var highlightedId: String? = nil
    let onTap: (TransactionListItem) -> Void
    var onToggleSelect: ((String) -> Void)? = nil
    var onRowAppear: ((TransactionListItem) -> Void)? = nil

    private var selectionActive: Bool { !selectedIds.isEmpty }

    var body: some View {
        ForEach(groups) { group in
            Section {
                ForEach(group.transactions, id: \.id) { transaction in
                    TransactionRow(
                        transaction: transaction,
                        isSelected: selectedIds.contains(transaction.id),
                        isHighlighted: transaction.id == highlightedId,
                        onTap: { handleTap(transaction) },
                        onLongPress: onToggleSelect.map { toggle in { toggle(transaction.id) } }
                    )
                    .onAppear { onRowAppear?(transaction) }
                }
            } header: {
                Text(group.label)
            }
        }
    }

    private func handleTap(_ transaction: TransactionListItem) {
        if selectionActive, let onToggleSelect {
            onToggleSelect(transaction.id)
        } else {
            onTap(transaction)
        }
    }
}
