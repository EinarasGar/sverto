import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct BindingSetupViewModelTests {
    @Test func failedLoadIsDistinctFromAnEmptyProviderResponse() async {
        let store = BindingSetupStore(noHandle: .init())
        store.setProviderResponse(.failure(.Parse(reason: "missing field accounts")))
        let model = BindingSetupViewModel(store: store, connectionId: "connection")

        await model.load()

        #expect(!model.loading)
        #expect(model.providerAccounts.isEmpty)
        #expect(model.loadError == "Couldn't load provider accounts. Pull down to try again.")
        #expect(model.error == nil)
    }

    @Test func successfulEmptyLoadHasNoFailure() async {
        let model = BindingSetupViewModel(store: BindingSetupStore(noHandle: .init()), connectionId: "connection")

        await model.load()

        #expect(!model.loading)
        #expect(model.providerAccounts.isEmpty)
        #expect(model.loadError == nil)
        #expect(model.error == nil)
    }

    @Test func successfulRetryClearsTheLoadFailureAndShowsAccounts() async {
        let store = BindingSetupStore(noHandle: .init())
        store.setProviderResponse(.failure(.Network(reason: "Offline")))
        let model = BindingSetupViewModel(store: store, connectionId: "connection")
        await model.load()
        let account = ProviderAccount(providerAccountId: "provider", displayName: "Savings", currency: "GBP", accountType: "Savings", accountNumberLast4: nil, balance: nil, history: .fetched)
        store.setProviderResponse(.success([account]))

        await model.load()

        #expect(model.loadError == nil)
        #expect(model.providerAccounts == [account])
        #expect(!model.loading)
    }

    @Test func failedRefreshKeepsPreviouslyLoadedAccountsAlongsideTheError() async {
        let store = BindingSetupStore(noHandle: .init())
        let account = ProviderAccount(providerAccountId: "provider", displayName: "Savings", currency: "GBP", accountType: "Savings", accountNumberLast4: nil, balance: nil, history: .fetched)
        store.setProviderResponse(.success([account]))
        let model = BindingSetupViewModel(store: store, connectionId: "connection")
        await model.load()
        store.setProviderResponse(.failure(.Network(reason: "Offline")))

        await model.load()

        #expect(model.providerAccounts == [account])
        #expect(model.loadError != nil)
        #expect(!model.loading)
    }

    @Test func failedBindingRemainsAnActionErrorWithoutChangingTheLoadState() async {
        let store = BindingSetupStore(noHandle: .init())
        store.setProviderResponse(.success([Self.providerAccount("provider")]))
        let model = BindingSetupViewModel(store: store, connectionId: "connection")
        await model.load()
        model.choose(providerAccountId: "provider", account: Self.svertoAccount("sverto"))

        let sessionId = await model.linkAccounts()

        #expect(sessionId == nil)
        #expect(model.error == "Couldn't link account")
        #expect(model.loadError == nil)
        #expect(model.choices.keys.contains("provider"))
    }

    @Test func importingSendsEveryChosenAccountWithTheStartDayAsUtcMidnight() async throws {
        let store = BindingSetupStore(noHandle: .init())
        store.setProviderResponse(.success(["a", "b", "c"].map(Self.providerAccount)))
        store.linksByImport = true
        let model = BindingSetupViewModel(store: store, connectionId: "connection")
        await model.load()
        model.choose(providerAccountId: "c", account: Self.svertoAccount("savings"))
        model.choose(providerAccountId: "a", account: Self.svertoAccount("current"))
        model.importStart = .date
        model.startDate = try #require(Calendar.current.date(from: DateComponents(year: 2026, month: 3, day: 5, hour: 23, minute: 30)))

        let sessionId = await model.linkAccounts()

        #expect(sessionId == "session")
        #expect(store.importCalls == [ImportCall(
            connectionId: "connection",
            accounts: [
                ConnectorImportAccount(providerAccountId: "a", svertoAccountId: "current"),
                ConnectorImportAccount(providerAccountId: "c", svertoAccountId: "savings"),
            ],
            importFrom: .date(date: 1_772_668_800)
        )])
    }

    private static func providerAccount(_ id: String) -> ProviderAccount {
        ProviderAccount(providerAccountId: id, displayName: id, currency: "EUR", accountType: nil, accountNumberLast4: nil, balance: nil, history: .fetched)
    }

    private static func svertoAccount(_ id: String) -> AccountListItem {
        AccountListItem(id: id, name: id, accountTypeId: 1, liquidityTypeId: 1, ownershipShare: 1, balance: nil, unrealizedGain: nil, holdingsCount: nil, suggestedCurrency: nil, property: nil, linkedPropertyId: nil)
    }
}

nonisolated private struct ImportCall: Equatable {
    let connectionId: String
    let accounts: [ConnectorImportAccount]
    let importFrom: ConnectorImportFrom
}

nonisolated private final class BindingSetupStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var providerResponse: Result<[ProviderAccount], ApiError> = .success([])
    private var importsByConnection = false
    private var recordedImports: [ImportCall] = []

    var linksByImport: Bool {
        get { lock.withLock { importsByConnection } }
        set { lock.withLock { importsByConnection = newValue } }
    }

    var importCalls: [ImportCall] { lock.withLock { recordedImports } }

    func setProviderResponse(_ response: Result<[ProviderAccount], ApiError>) {
        lock.withLock { providerResponse = response }
    }

    override func listConnectorProviderAccounts(connectionId: String) async throws -> [ProviderAccount] {
        try lock.withLock { providerResponse }.get()
    }

    override func listConnectorBindings() async throws -> [ConnectorBinding] { [] }
    override func listSvertoAccounts() async throws -> [AccountListItem] { [] }

    override func listConnectorConnections() async throws -> [ConnectorConnection] {
        [ConnectorConnection(id: "connection", providerKind: "enablebanking", credentialMode: .stored, linksByImport: linksByImport, providerKeyId: nil, status: "active", consentExpiresAt: nil, createdAt: 0)]
    }

    override func createConnectorBinding(
        connectionId: String,
        svertoAccountId: String,
        providerAccountId: String?,
        importFrom: ConnectorImportFrom
    ) async throws -> String {
        throw ApiError.Server(reason: "Couldn't link account", status: 400)
    }

    override func createConnectorImport(
        connectionId: String,
        accounts: [ConnectorImportAccount],
        importFrom: ConnectorImportFrom
    ) async throws -> String {
        lock.withLock { recordedImports.append(ImportCall(connectionId: connectionId, accounts: accounts, importFrom: importFrom)) }
        return "session"
    }
}
