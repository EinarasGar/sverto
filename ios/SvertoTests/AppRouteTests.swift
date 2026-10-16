import SwiftUI
import Testing
import UIKit
@testable import Sverto

private let everyRoute: [AppRoute] = [
    .transactionDetail(id: "tx-1"),
    .createTransaction(typeKey: "regular_transaction", quickUploadId: nil),
    .createTransaction(typeKey: "regular_transaction", quickUploadId: "qu-1"),
    .editTransaction(typeKey: "asset_purchase", txId: "tx-2"),
    .createTransactionGroup(quickUploadId: nil, fromSelection: false),
    .createTransactionGroup(quickUploadId: "qu-2", fromSelection: true),
    .editTransactionGroup(groupId: "grp-1"),
    .groupAddTransaction(typeKey: "cash_transfer_in"),
    .groupEditTransaction(typeKey: "cash_transfer_out", index: 2),
    .accountDetail(accountId: "acc-1", accountName: "Main", accountTypeId: 3),
    .accountAssetDetail(accountId: "acc-1", accountName: "ISA", assetId: 12),
    .accountTransactions(accountId: "acc-1"),
    .assetTransactions(assetId: 12, accountId: nil, title: "AAPL", subtitle: nil),
    .assetTransactions(assetId: 12, accountId: "acc-1", title: "AAPL", subtitle: "ISA"),
    .addAccount,
    .editAccount(accountId: "acc-1"),
    .assetOverview(assetId: 12, userAsset: false),
    .assetOverview(assetId: 13, userAsset: true),
    .customAssets,
    .customCategories,
    .connectors,
    .connectorDetail(providerKind: "truelayer"),
    .connectionDetail(connectionId: "conn-1"),
    .bindingSetup(connectionId: "conn-1"),
    .providerAccountDetail(connectionId: "conn-1", providerAccountId: "pa-1", displayName: "Current", currency: "GBP"),
    .providerAccountDetail(connectionId: "conn-1", providerAccountId: "pa-2", displayName: "Wallet", currency: nil),
    .connectTrueLayer(code: nil, state: nil, error: nil),
    .connectTrueLayer(code: "c", state: "s", error: "e"),
    .connectEnableBanking(code: nil, state: nil, error: nil),
    .connectEnableBanking(code: "c", state: "s", error: "e"),
    .connectTrading212,
]

private func caseName(_ route: AppRoute) -> String {
    let description = String(describing: route)
    return String(description.prefix { $0 != "(" })
}

struct AppRouteTests {
    @Test func everyCaseHasARouteDestination() {
        for route in everyRoute {
            let destination = RouteDestination(route: route)
            #expect(destination.route == route)
        }
    }

    @Test func coversAllTwentyFourCases() {
        #expect(Set(everyRoute.map(caseName)).count == 24)
    }

    @Test func routesAreDistinguishedByPayload() {
        #expect(Set(everyRoute).count == everyRoute.count)
        #expect(AppRoute.createTransaction(typeKey: "a", quickUploadId: nil) != .createTransaction(typeKey: "a", quickUploadId: "x"))
        #expect(AppRoute.assetOverview(assetId: 1, userAsset: true) != .assetOverview(assetId: 1, userAsset: false))
        #expect(AppRoute.connectTrueLayer(code: nil, state: nil, error: nil) != .connectEnableBanking(code: nil, state: nil, error: nil))
    }

    @Test func routesAreUsableAsNavigationPathElements() {
        var path: [AppRoute] = []
        path.append(.connectors)
        path.append(.connectTrading212)

        #expect(path == [.connectors, .connectTrading212])
        #expect(path.last == .connectTrading212)
    }
}

struct GroupChildModeTests {
    @Test func addAndEditAreDistinctAndEditCarriesIndex() {
        #expect(GroupChildMode.add != .edit(index: 0))
        #expect(GroupChildMode.edit(index: 1) != .edit(index: 2))
        #expect(GroupChildMode.edit(index: 1) == .edit(index: 1))
    }
}

struct AppTabTests {
    @Test func hasFourTabsInDisplayOrder() {
        #expect(AppTab.allCases == [.portfolio, .transactions, .accounts, .chat])
    }

    @Test func titlesMatchAndroidTabs() {
        #expect(AppTab.allCases.map(\.title) == ["Portfolio", "Transactions", "Accounts", "Myra"])
    }

    @Test func everyTabHasAUniqueSymbol() {
        let symbols = AppTab.allCases.map(\.systemImage)

        #expect(symbols.allSatisfy { !$0.isEmpty })
        #expect(Set(symbols).count == symbols.count)
        for symbol in symbols {
            #expect(UIImage(systemName: symbol) != nil)
        }
    }

    @Test func rawValuesAreStableIdentifiers() {
        #expect(AppTab(rawValue: "portfolio") == .portfolio)
        #expect(AppTab(rawValue: "chat") == .chat)
        #expect(AppTab(rawValue: "myra") == nil)
    }
}
