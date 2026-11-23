import Foundation
import Observation
import SvertoCore

@Observable
final class BindingSetupViewModel {
    private(set) var providerAccounts: [ProviderAccount] = []
    private(set) var bindings: [String: ConnectorBinding] = [:]
    private(set) var svertoAccounts: [AccountListItem] = []
    private(set) var linksByImport = false
    private(set) var loading = true
    private(set) var linking = false
    private(set) var error: String?
    private(set) var loadError: String?
    private(set) var choices: [String: AccountListItem] = [:]
    var importStart: ImportStart = .lastEntry
    var startDate = Date()

    private let store: AppStore
    private let connectionId: String
    private var resumed = false
    private var pollTask: Task<Void, Never>?

    private static let historyPollInterval: Duration = .seconds(5)

    init(store: AppStore, connectionId: String) {
        self.store = store
        self.connectionId = connectionId
    }

    var hasUnlinkedAccounts: Bool {
        providerAccounts.contains { bindings[$0.providerAccountId] == nil }
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
            let provider = try await store.listConnectorProviderAccounts(connectionId: connectionId)
            let bindings = try await store.listConnectorBindings()
                .filter { $0.connectionId == connectionId }
                .map { ($0.providerAccountId, $0) }
            let accounts = try await store.listSvertoAccounts()
            let connection = try await store.listConnectorConnections().first { $0.id == connectionId }
            providerAccounts = provider
            self.bindings = Dictionary(bindings, uniquingKeysWith: { _, last in last })
            svertoAccounts = accounts
            linksByImport = connection?.linksByImport == true
            choices = choices.filter { self.bindings[$0.key] == nil }
            loadError = nil
            loading = false
            if resumed, provider.contains(where: { $0.history == .fetching }) { poll() }
        } catch {
            loading = false
            loadError = "Couldn't load provider accounts. Pull down to try again."
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

    func choose(providerAccountId: String, account: AccountListItem) {
        choices[providerAccountId] = account
    }

    func clearChoice(providerAccountId: String) {
        choices[providerAccountId] = nil
    }

    func linkAccounts() async -> String? {
        guard !linking else { return nil }
        linking = true
        error = nil
        defer { linking = false }
        let importFrom = importStart.importFrom(startDate: startDate)
        let chosen = providerAccounts.compactMap { account in
            choices[account.providerAccountId].map { (providerAccountId: account.providerAccountId, account: $0) }
        }
        if linksByImport {
            do {
                return try await store.createConnectorImport(
                    connectionId: connectionId,
                    accounts: chosen.map {
                        ConnectorImportAccount(providerAccountId: $0.providerAccountId, svertoAccountId: $0.account.id)
                    },
                    importFrom: importFrom
                )
            } catch {
                self.error = error.friendlyMessage
                return nil
            }
        }
        do {
            for choice in chosen {
                _ = try await store.createConnectorBinding(
                    connectionId: connectionId,
                    svertoAccountId: choice.account.id,
                    providerAccountId: choice.providerAccountId,
                    importFrom: importFrom
                )
                choices[choice.providerAccountId] = nil
            }
        } catch {
            self.error = error.friendlyMessage
        }
        await load()
        return nil
    }
}
