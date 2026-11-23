import SvertoCore
import SwiftUI

struct ProviderAccountDetailScreen: View {
    let connectionId: String
    let providerAccountId: String
    let displayName: String
    let currency: String?
    let fromLinkSetup: Bool
    @Environment(AppServices.self) private var services

    var body: some View {
        ProviderAccountDetailContent(
            store: services.store,
            connectionId: connectionId,
            providerAccountId: providerAccountId,
            displayName: displayName,
            currency: currency,
            fromLinkSetup: fromLinkSetup
        )
    }
}

private struct ProviderAccountDetailContent: View {
    @State private var model: ProviderAccountDetailViewModel
    @State private var isPickingAccount = false
    @State private var chosenAccount: AccountListItem?
    @Environment(ShellState.self) private var shell
    private let connectionId: String
    private let providerAccountId: String
    private let displayName: String
    private let currency: String?
    private let fromLinkSetup: Bool

    init(
        store: AppStore,
        connectionId: String,
        providerAccountId: String,
        displayName: String,
        currency: String?,
        fromLinkSetup: Bool
    ) {
        _model = State(initialValue: ProviderAccountDetailViewModel(
            store: store,
            connectionId: connectionId,
            providerAccountId: providerAccountId
        ))
        self.connectionId = connectionId
        self.providerAccountId = providerAccountId
        self.displayName = displayName
        self.currency = currency
        self.fromLinkSetup = fromLinkSetup
    }

    private var statusLine: String {
        let linked = switch model.binding {
        case let binding? where binding.isWaitingForImport: "Being imported into \(binding.svertoAccountName)"
        case let binding?: "Linked to \(binding.svertoAccountName)"
        case nil: "Not linked"
        }
        return [currency, linked, model.history?.label].compactMap { $0 }.joined(separator: " · ")
    }

    private var canLink: Bool {
        fromLinkSetup && !model.loading && model.binding == nil
    }

    var body: some View {
        List {
            Section {
                Text(statusLine)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .listRowBackground(Color.clear)
                if let error = model.error {
                    Text(error)
                        .font(.subheadline)
                        .foregroundStyle(.red)
                        .listRowBackground(Color.clear)
                }
            }
            if model.loading, model.transactions.isEmpty {
                Section { BindingRowsSkeleton() }
            } else if model.transactions.isEmpty, model.error == nil {
                Section {
                    Group {
                        if model.history == .fetching {
                            EmptyState(
                                title: "Fetching history…",
                                systemImage: "arrow.down.circle",
                                description: "Your bank is sending this account's transactions. They appear here as they arrive."
                            )
                        } else {
                            EmptyState(
                                title: "No transactions fetched yet",
                                systemImage: "tray",
                                description: "Transactions appear here after this account's history has been fetched."
                            )
                        }
                    }
                    .listRowBackground(Color.clear)
                }
            } else {
                Section {
                    ForEach(Array(model.transactions.enumerated()), id: \.offset) { _, transaction in
                        TransactionPreviewRow(transaction: transaction)
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(ConnectorNames.humanizeAccountName(displayName))
        .navigationBarTitleDisplayMode(.inline)
        .safeAreaInset(edge: .bottom) {
            if canLink {
                ConnectorHeroButton(title: "Link account") { isPickingAccount = true }
            }
        }
        .onAppear { model.resume() }
        .onDisappear { model.pause() }
        .refreshable { await model.load() }
        .sheet(isPresented: $isPickingAccount, onDismiss: handOverChoice) {
            BindingAccountPickerSheet(accounts: model.svertoAccounts) { chosenAccount = $0 }
        }
    }

    private func handOverChoice() {
        guard let chosenAccount else { return }
        self.chosenAccount = nil
        shell.pendingBindingChoice = PendingBindingChoice(
            connectionId: connectionId,
            providerAccountId: providerAccountId,
            account: chosenAccount
        )
        shell.pop()
    }
}

private struct TransactionPreviewRow: View {
    let transaction: ProviderAccountTransaction
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var description: String {
        let trimmed = transaction.description.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? "(no description)" : transaction.description
    }

    private var detail: String {
        var parts = [ConnectorTimeFormat.absoluteDate(transaction.date)]
        if let quantity = transaction.quantity, let asset = transaction.assetIdentifier {
            parts.append("\(ConnectorNames.quantity(quantity)) × \(asset)")
        }
        return parts.joined(separator: " · ")
    }

    private var amount: String {
        Money.format(transaction.amount, ticker: transaction.currency, signed: true)
    }

    var body: some View {
        contentLayout {
            VStack(alignment: .leading, spacing: 2) {
                Text(description)
                    .fixedSize(horizontal: false, vertical: true)
                Text(detail)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
            if !dynamicTypeSize.isAccessibilitySize {
                Spacer(minLength: 8)
            }
            AmountText(amount)
                .layoutPriority(1)
                .accessibilityValue(transaction.currency)
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }

    private var contentLayout: AnyLayout {
        dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(alignment: .center, spacing: 12))
    }
}
