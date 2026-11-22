import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct PropertyAccountsListTests {
    @Test func linkedLoansAreLeftOutOfTheListButUnlinkedLoansStay() async throws {
        let model = try await startedModel()

        let listed = model.sections.flatMap { $0.accounts.map(\.id) }

        #expect(Set(listed) == ["current", "house", "car-loan"])
        #expect(!listed.contains("mortgage"))
    }

    @Test func editingAPropertysAccountLocksItsType() async {
        for (accountId, locked) in [("mortgage", true), ("car-loan", false)] {
            let model = AddAccountViewModel(store: AccountsListStore(noHandle: .init()), editingAccountId: accountId)

            await model.start()

            #expect(model.belongsToProperty == locked)
            #expect(model.selectedTypeId == 7)
        }
    }

    @Test func vehiclesSortNextToRealEstate() {
        #expect(AccountTypeVisuals.orderedTypeIds([8, 12, 1, 9, 7]) == [1, 9, 12, 7, 8])
        #expect(AccountTypeVisuals.label(12) == "Vehicle")
    }

    private func startedModel() async throws -> AccountsViewModel {
        let store = AccountsListStore(noHandle: .init())
        store.accounts = [
            makeAccount("current", type: 1),
            makeAccount("house", type: 9, property: makeSummary(loanIds: ["mortgage"])),
            makeAccount("mortgage", type: 7, linkedPropertyId: "property-1"),
            makeAccount("car-loan", type: 8),
        ]
        let model = AccountsViewModel(store: store)
        await model.start()
        try await waitUntil { model.state.accounts.count == 4 }
        return model
    }

    private func waitUntil(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while !condition(), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(condition())
    }
}

nonisolated func makeAccount(
    _ id: String,
    type: Int32,
    share: Double = 1,
    balance: Double? = nil,
    property: PropertySummary? = nil,
    linkedPropertyId: String? = nil
) -> AccountListItem {
    AccountListItem(
        id: id,
        name: id,
        accountTypeId: type,
        liquidityTypeId: 1,
        ownershipShare: share,
        balance: balance,
        unrealizedGain: nil,
        holdingsCount: nil,
        suggestedCurrency: nil,
        property: property,
        linkedPropertyId: linkedPropertyId
    )
}

nonisolated func makeSummary(
    loanIds: [String] = [],
    value: Double? = nil,
    loanBalance: Double? = nil,
    equity: Double? = nil,
    equityShare: Double? = nil,
    loanToValue: Double? = nil
) -> PropertySummary {
    PropertySummary(
        propertyId: "property-1",
        kind: .realEstate,
        assetId: 7,
        currencyAssetId: 45,
        loanAccountIds: loanIds,
        value: value,
        loanBalance: loanBalance,
        equity: equity,
        equityShare: equityShare,
        loanToValue: loanToValue
    )
}

nonisolated private final class AccountsListStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var items: [AccountListItem] = []

    var accounts: [AccountListItem] {
        get { lock.withLock { items } }
        set { lock.withLock { items = newValue } }
    }

    override func observeAccounts(observer: any AccountsObserver) {
        observer.onAccountsChanged(state: AccountsState(
            isLoading: false,
            isLoadingBalances: false,
            error: nil,
            accounts: accounts,
            totalNetWorth: 0,
            baseTicker: "GBP"
        ))
    }

    override func unobserveAccounts() {}
    override func loadAccounts() async {}

    override func getAccountTypes() async throws -> [AccountTypeItem] {
        [AccountTypeItem(id: 7, name: "Mortgage")]
    }

    override func getAccount(accountId: String) async throws -> AccountEditModel {
        AccountEditModel(
            id: accountId,
            name: accountId,
            accountTypeId: 7,
            liquidityTypeId: 1,
            ownershipShare: 0.5,
            identifiers: [],
            belongsToProperty: accountId == "mortgage"
        )
    }
}
