import Foundation
import Observation
import SvertoCore

@Observable
final class AccountDetailViewModel {
    private(set) var state: AccountDetailState
    private(set) var isRefreshing = false
    private(set) var syncState = AccountSyncState()

    let accountId: String
    private let accountName: String
    private let accountTypeId: Int32
    private let store: AppStore
    private var bridge: AccountDetailObserverBridge?
    private var hasLoaded = false

    init(store: AppStore, accountId: String, accountName: String, accountTypeId: Int32) {
        self.store = store
        self.accountId = accountId
        self.accountName = accountName
        self.accountTypeId = accountTypeId
        state = AccountDetailState(
            isLoading: true,
            error: nil,
            accountId: accountId,
            accountName: accountName,
            accountTypeId: accountTypeId,
            chartData: [],
            holdings: [],
            balances: [],
            cashBalance: 0,
            totalValue: 0,
            totalCostBasis: 0,
            unrealizedGains: 0,
            realizedGains: 0,
            totalFees: 0,
            recentTransactions: [],
            baseTicker: ""
        )
    }

    var isInvestmentAccount: Bool { AccountTypeVisuals.isInvestment(accountTypeId) }
    var showsBalances: Bool { state.balances.count > (isInvestmentAccount ? 0 : 1) }
    var showsLoading: Bool { state.isLoading && !isRefreshing }

    func start() async {
        observeIfNeeded()
        guard !hasLoaded else { return }
        hasLoaded = true
        async let detail: Void = store.loadAccountDetail(
            accountId: accountId,
            accountName: accountName,
            accountTypeId: accountTypeId
        )
        async let sync: Void = loadSyncInfo()
        _ = await (detail, sync)
    }

    func stop() {
        guard bridge != nil else { return }
        bridge = nil
        store.unobserveAccountDetail()
    }

    func refresh() async {
        isRefreshing = true
        await store.refreshAccountDetail()
        isRefreshing = false
    }

    func delete() async -> String? {
        do {
            try await store.deleteAccount(accountId: accountId)
            return nil
        } catch {
            return error.friendlyMessage
        }
    }

    func loadSyncInfo() async {
        do {
            let connections = try await store.listConnectorConnections()
            let bindings = try await store.listConnectorBindings()
            let credentialModes = Dictionary(
                connections.map { ($0.id, $0.credentialMode) },
                uniquingKeysWith: { _, latest in latest }
            )
            let targets = bindings
                .filter { $0.svertoAccountId == accountId }
                .compactMap { binding in
                    credentialModes[binding.connectionId].map { AccountSyncTarget(binding: binding, credentialMode: $0) }
                }
            syncState.targets = targets
            syncState.lastSyncAt = targets.compactMap(\.binding.lastSyncAt).max()
        } catch {}
    }

    func sync() async {
        syncState.isSyncing = true
        syncState.message = nil
        var queued = 0
        var imported: Int64 = 0
        var failure: String?
        for target in syncState.targets where target.isActive {
            do {
                let outcome = try await store.syncConnectorBinding(
                    bindingId: target.binding.id,
                    connectionId: target.binding.connectionId,
                    credentialMode: target.credentialMode
                )
                if outcome.status == "queued" {
                    queued += 1
                } else {
                    imported += outcome.report?.newTransactions ?? 0
                }
            } catch ApiError.MissingLocalCredential {
                syncState.isSyncing = false
                syncState.promptConnectionId = target.binding.connectionId
                return
            } catch {
                failure = error.friendlyMessage
            }
        }
        syncState.isSyncing = false
        syncState.message = failure ?? Self.syncSummary(queued: queued, imported: imported)
        await refresh()
        await loadSyncInfo()
    }

    func saveTransientKey(connectionId: String, key: String) async {
        let store = self.store
        await Task.detached { store.saveConnectorCredential(connectionId: connectionId, secret: key) }.value
        syncState.promptConnectionId = nil
        await sync()
    }

    func dismissKeyPrompt() {
        syncState.promptConnectionId = nil
    }

    func consumeSyncMessage() {
        syncState.message = nil
    }

    private static func syncSummary(queued: Int, imported: Int64) -> String {
        if queued > 0, imported == 0 {
            return "Sync started"
        }
        if imported > 0 {
            return "\(imported) new transactions"
        }
        return "Up to date"
    }

    private func observeIfNeeded() {
        guard bridge == nil else { return }
        let bridge = AccountDetailObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in self?.state = state }
        }
        self.bridge = bridge
        store.observeAccountDetail(observer: bridge)
    }
}
