import SvertoCore
import SwiftUI

struct ScopedTransactionsScreen: View {
    let scope: TransactionListScope
    let title: String
    var subtitle: String?

    @Environment(AppServices.self) private var services

    var body: some View {
        ScopedTransactionsList(store: services.store, scope: scope, title: title, subtitle: subtitle)
    }
}

private struct ScopedTransactionsList: View {
    @State private var model: ScopedTransactionsViewModel
    @Environment(ShellState.self) private var shell
    private let title: String
    private let subtitle: String?

    init(store: AppStore, scope: TransactionListScope, title: String, subtitle: String?) {
        _model = State(initialValue: ScopedTransactionsViewModel(store: store, scope: scope))
        self.title = title
        self.subtitle = subtitle
    }

    var body: some View {
        content
            .navigationTitle(title)
            .navigationSubtitle(subtitle ?? "")
            .navigationBarTitleDisplayMode(.inline)
            .task { await model.start() }
            .onDisappear { model.stop() }
    }

    @ViewBuilder
    private var content: some View {
        if model.showsLoading {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.screenBackground)
        } else {
            transactionList
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if let error = model.state.error {
            ContentUnavailableView {
                Label("Couldn't load transactions", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            }
        } else {
            ContentUnavailableView(
                "No transactions",
                systemImage: "list.bullet.rectangle.portrait",
                description: Text("Matching transactions will appear here.")
            )
        }
    }

    private var transactionList: some View {
        List {
            if model.state.items.isEmpty {
                Section {
                    emptyState
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 40)
                        .listRowBackground(Color.clear)
                        .listRowSeparator(.hidden)
                }
            }
            ForEach(model.sections) { section in
                Section(section.label) {
                    ForEach(section.transactions, id: \.id) { transaction in
                        Button {
                            shell.showTransactionDetail(transaction, isInGroup: false)
                        } label: {
                            AccountTransactionRow(transaction: transaction)
                        }
                        .task { await model.loadMoreIfNeeded(appearing: transaction) }
                    }
                }
            }
            if model.state.isLoadingMore {
                Section {
                    ProgressView()
                        .frame(maxWidth: .infinity)
                        .listRowBackground(Color.clear)
                        .accessibilityLabel("Loading more transactions")
                }
            }
        }
        .listStyle(.insetGrouped)
        .refreshable { await model.refresh() }
    }
}
