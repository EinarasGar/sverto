import Foundation
import Observation
import SvertoCore

@Observable
final class ProviderAccountDetailViewModel {
    private(set) var transactions: [ProviderAccountTransaction] = []
    private(set) var svertoAccounts: [AccountListItem] = []
    private(set) var binding: ConnectorBinding?
    private(set) var history: ProviderAccountHistory?
    private(set) var loading = true
    private(set) var error: String?

    private let store: AppStore
    private let connectionId: String
    private let providerAccountId: String

    private var resumed = false
    private var pollTask: Task<Void, Never>?

    private static let historyPollInterval: Duration = .seconds(5)

    init(store: AppStore, connectionId: String, providerAccountId: String) {
        self.store = store
        self.connectionId = connectionId
        self.providerAccountId = providerAccountId
    }

    func resume() {
        resumed = true
        Task { await load() }
    }

    func pause() {
        resumed = false
        pollTask?.cancel()
    }

    func load() async {
        do {
            let transactions = try await store.listConnectorProviderAccountTransactions(
                connectionId: connectionId,
                providerAccountId: providerAccountId
            )
            let history = try await store.listConnectorProviderAccounts(connectionId: connectionId)
                .first { $0.providerAccountId == providerAccountId }?
                .history
            let binding = try await store.listConnectorBindings()
                .first { $0.connectionId == connectionId && $0.providerAccountId == providerAccountId }
            let accounts = try await store.listSvertoAccounts()
            self.transactions = transactions
            self.history = history
            svertoAccounts = accounts
            self.binding = binding
            error = nil
            loading = false
            if resumed, history == .fetching { poll() }
        } catch {
            loading = false
            self.error = error.friendlyMessage
        }
    }

    private func poll() {
        pollTask?.cancel()
        pollTask = Task { [weak self] in
            try? await Task.sleep(for: Self.historyPollInterval)
            guard !Task.isCancelled, let self else { return }
            await load()
        }
    }
}
