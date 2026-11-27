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
    case propertyDetail(propertyId: String, name: String)
    case addProperty
    case propertyValuations(assetId: Int32, currencyAssetId: Int32, currencyTicker: String)
    case assetOverview(assetId: Int32, userAsset: Bool)
    case currencyDetail(assetId: Int32, name: String, accountId: String? = nil)
    case currencyTransactions(assetId: Int32, title: String)
    case customAssets
    case customCategories
    case connectors
    case connectorDetail(providerKind: String)
    case connectionDetail(connectionId: String)
    case bindingSetup(connectionId: String)
    case providerAccountDetail(connectionId: String, providerAccountId: String, displayName: String, currency: String?, fromLinkSetup: Bool = false)
    case connectTrueLayer(code: String?, state: String?, error: String?)
    case connectEnableBanking(code: String?, state: String?, error: String?)
    case connectTrading212
    case imports
    case importSession(id: String, pickFiles: Bool = false)
    case importItems(sessionId: String)

    var takesFocus: Bool {
        switch self {
        case .createTransaction, .editTransaction, .createTransactionGroup, .editTransactionGroup,
             .groupAddTransaction, .groupEditTransaction, .addAccount, .editAccount, .addProperty,
             .connectTrueLayer, .connectEnableBanking, .connectTrading212:
            true
        default:
            false
        }
    }
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
