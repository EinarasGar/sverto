import Foundation

enum AppRoute: Hashable {
    case transactionDetail(id: String)
    case createTransaction(typeKey: String, quickUploadId: String?)
    case editTransaction(typeKey: String, txId: String)
    case createTransactionGroup(quickUploadId: String?, fromSelection: Bool)
    case editTransactionGroup(groupId: String)
    case groupAddTransaction(typeKey: String)
    case groupEditTransaction(typeKey: String, index: Int)
    case accountDetail(accountId: String, accountName: String, accountTypeId: Int32)
    case accountAssetDetail(accountId: String, accountName: String, assetId: Int32)
    case accountTransactions(accountId: String)
    case assetTransactions(assetId: Int32, accountId: String?, title: String, subtitle: String?)
    case addAccount
    case editAccount(accountId: String)
    case assetOverview(assetId: Int32, userAsset: Bool)
    case customAssets
    case customCategories
    case connectors
    case connectorDetail(providerKind: String)
    case connectionDetail(connectionId: String)
    case bindingSetup(connectionId: String)
    case providerAccountDetail(connectionId: String, providerAccountId: String, displayName: String, currency: String?)
    case connectTrueLayer(code: String?, state: String?, error: String?)
    case connectEnableBanking(code: String?, state: String?, error: String?)
    case connectTrading212
}

enum GroupChildMode: Hashable {
    case add
    case edit(index: Int)
}

enum AppTab: String, CaseIterable, Hashable {
    case portfolio
    case transactions
    case accounts
    case chat

    var title: String {
        switch self {
        case .portfolio: "Portfolio"
        case .transactions: "Transactions"
        case .accounts: "Accounts"
        case .chat: "Myra"
        }
    }

    var systemImage: String {
        switch self {
        case .portfolio: "chart.line.uptrend.xyaxis"
        case .transactions: "list.bullet.rectangle.portrait"
        case .accounts: "building.columns"
        case .chat: "sparkles"
        }
    }
}
