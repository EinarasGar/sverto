import SwiftUI
import SvertoCore

struct TransactionDetailScreen: View {
    let transactionId: String

    @Environment(ShellState.self) private var shell

    var body: some View {
        if let detail = shell.transactionCache[transactionId] {
            TransactionDetailContent(detail: detail)
        } else {
            ContentUnavailableView("Transaction not available", systemImage: "doc.text.magnifyingglass")
                .navigationBarTitleDisplayMode(.inline)
        }
    }
}

private struct TransactionDetailContent: View {
    let detail: TransactionDetailState

    @Environment(ShellState.self) private var shell
    @Environment(TransactionsViewModel.self) private var actions
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var visibility: TransactionVisibility
    @State private var isDeleteConfirmationPresented = false

    init(detail: TransactionDetailState) {
        self.detail = detail
        _visibility = State(initialValue: detail.transaction.visibility)
    }

    private var transaction: TransactionListItem { detail.transaction }

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                TransactionDetailHero(transaction: transaction)
                quickActions
                    .padding(.top, 20)
                    .padding(.horizontal, Layout.cardPadding)
                detailsCard
                    .padding(.top, 20)
                    .padding(.horizontal, Layout.cardPadding)
                if transaction.isGroup, !transaction.children.isEmpty {
                    childrenSection
                        .padding(.top, 24)
                        .padding(.horizontal, Layout.cardPadding)
                }
            }
            .padding(.bottom, 24)
        }
        .background(.screenBackground)
        .navigationTitle(transaction.typeLabel)
        .navigationBarTitleDisplayMode(.inline)
        .modifier(deleteDialog(.screen))
        .sensoryFeedback(.success, trigger: visibility)
    }

    private var quickActions: some View {
        let layout = dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(spacing: 12))
            : AnyLayout(HStackLayout(alignment: .top, spacing: 12))
        return layout {
            if visibility == .ghost {
                TransactionQuickAction(systemImage: "checkmark.circle", label: "Reviewed", action: markReviewed)
            }
            TransactionQuickAction(systemImage: "pencil", label: "Edit", isPrimary: true, action: edit)
            TransactionQuickAction(systemImage: "trash", label: "Delete", isDestructive: true) {
                isDeleteConfirmationPresented = true
            }
            .modifier(deleteDialog(.trigger))
        }
        .frame(maxWidth: .infinity)
    }

    private func deleteDialog(_ source: DialogSource) -> some ViewModifier {
        ConfirmationDialog(source, title: "Delete transaction?", isPresented: $isDeleteConfirmationPresented) {
            Button("Delete", role: .destructive, action: deleteAndPop)
        } message: {
            Text("This action can't be undone.")
        }
    }

    private var detailsCard: some View {
        GroupedCard {
            VStack(spacing: 0) {
                TransactionDetailRow(label: "Date", value: TransactionDetailDate.string(epochSeconds: transaction.date))
                if !transaction.accountName.isEmpty {
                    RowDivider()
                    if let destination = transaction.secondaryAccountName, !destination.isEmpty {
                        TransactionDetailRow(label: "From", value: transaction.accountName)
                        RowDivider()
                        TransactionDetailRow(label: "To", value: destination)
                    } else {
                        TransactionDetailRow(label: "Account", value: transaction.accountName)
                    }
                }
                if !transaction.assetDisplay.isEmpty {
                    RowDivider()
                    TransactionDetailRow(label: "Asset", value: transaction.assetDisplay)
                }
                if !transaction.categoryName.isEmpty {
                    RowDivider()
                    TransactionDetailRow(label: "Category", value: transaction.categoryName)
                }
                RowDivider()
                TransactionDetailRow(label: "Type", value: transaction.typeLabel)
            }
        }
    }

    private var childrenSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Transactions")
            GroupedCard {
                VStack(spacing: 0) {
                    ForEach(Array(transaction.children.enumerated()), id: \.element.id) { index, child in
                        TransactionRow(transaction: child) {
                            shell.showTransactionDetail(child, isInGroup: true)
                        }
                        .padding(.horizontal, Layout.cardPadding)
                        .padding(.vertical, 6)
                        if index < transaction.children.count - 1 {
                            RowDivider()
                        }
                    }
                }
            }
        }
    }

    private func markReviewed() {
        visibility = .default
        let id = transaction.id
        Task { await actions.markReviewed(id: id) }
    }

    private func edit() {
        if transaction.isGroup {
            shell.push(.editTransactionGroup(groupId: transaction.id))
        } else {
            shell.push(.editTransaction(typeKey: apiTypeToConfigKey(transaction.transactionType), txId: transaction.id))
        }
    }

    private func deleteAndPop() {
        let id = transaction.id
        let isGroup = transaction.isGroup
        Task {
            let deleted = isGroup ? await actions.deleteTransactionGroup(id: id) : await actions.deleteTransaction(id: id)
            if deleted {
                shell.transactionCache.removeValue(forKey: id)
            }
        }
        shell.pop()
    }
}
