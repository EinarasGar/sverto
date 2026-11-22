import SvertoCore
import SwiftUI

struct AccountsScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        AccountsList(store: services.store)
    }
}

private struct AccountsList: View {
    @State private var model: AccountsViewModel
    @Environment(ShellState.self) private var shell

    init(store: AppStore) {
        _model = State(initialValue: AccountsViewModel(store: store))
    }

    var body: some View {
        content
            .navigationTitle("Accounts")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button("Add account", systemImage: "building.columns") {
                            shell.push(.addAccount)
                        }
                        Button("Add property", systemImage: "house") {
                            shell.push(.addProperty)
                        }
                    } label: {
                        Image(systemName: "plus")
                    }
                    .tint(.primary)
                    .accessibilityLabel("Add")
                }
                ProfileToolbar()
            }
            .task { await model.start() }
            .onDisappear { model.stop() }
    }

    @ViewBuilder
    private var content: some View {
        if model.showsSkeleton {
            AccountsSkeleton()
        } else {
            accountsList
        }
    }

    private var accountsList: some View {
        List {
            if model.state.accounts.isEmpty {
                Section {
                    emptyState
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 40)
                        .listRowBackground(Color.clear)
                        .listRowSeparator(.hidden)
                }
            }
            ForEach(model.sections) { section in
                Section(AccountTypeVisuals.label(section.typeId)) {
                    ForEach(section.accounts, id: \.id) { account in
                        AccountCard(account: account, baseTicker: model.state.baseTicker) {
                            if let property = account.property {
                                shell.push(.propertyDetail(propertyId: property.propertyId, name: account.name))
                            } else {
                                shell.push(.accountDetail(
                                    accountId: account.id,
                                    accountName: account.name,
                                    accountTypeId: account.accountTypeId
                                ))
                            }
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .refreshable { await model.refresh() }
    }

    @ViewBuilder
    private var emptyState: some View {
        if let error = model.state.error {
            ContentUnavailableView {
                Label("Couldn't load accounts", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") {
                    Task { await model.retry() }
                }
                .buttonStyle(.borderedProminent)
            }
        } else {
            ContentUnavailableView(
                "No accounts yet",
                systemImage: "building.columns",
                description: Text("Add an account to start tracking your balances.")
            )
        }
    }

}

private struct AccountsSkeleton: View {
    var body: some View {
        VStack(spacing: 12) {
            ForEach(0..<5, id: \.self) { _ in
                SkeletonBlock(height: 72)
            }
            Spacer()
        }
        .padding(.horizontal, Layout.cardPadding)
        .padding(.top, 8)
        .background(.screenBackground)
        .accessibilityLabel("Loading accounts")
    }
}
