import SvertoCore
import SwiftUI

struct RouteDestination: View {
    let route: AppRoute

    var body: some View {
        screen.offlineBannerInset()
    }

    @ViewBuilder
    private var screen: some View {
        switch route {
        case let .transactionDetail(id):
            TransactionDetailScreen(transactionId: id)
        case let .createTransaction(typeKey, quickUploadId):
            CreateTransactionScreen(typeKey: typeKey, quickUploadId: quickUploadId)
        case let .editTransaction(typeKey, txId):
            CreateTransactionScreen(typeKey: typeKey, quickUploadId: nil, editTransactionId: txId)
        case let .createTransactionGroup(quickUploadId, fromSelection):
            CreateTransactionGroupScreen(quickUploadId: quickUploadId, fromSelection: fromSelection)
        case let .editTransactionGroup(groupId):
            CreateTransactionGroupScreen(quickUploadId: nil, fromSelection: false, editGroupId: groupId)
        case let .groupAddTransaction(typeKey):
            CreateTransactionScreen(typeKey: typeKey, quickUploadId: nil, groupChildMode: .add)
        case let .groupEditTransaction(typeKey, index):
            CreateTransactionScreen(typeKey: typeKey, quickUploadId: nil, groupChildMode: .edit(index: index))
        case let .accountDetail(accountId, accountName, accountTypeId):
            AccountDetailScreen(accountId: accountId, accountName: accountName, accountTypeId: accountTypeId)
        case let .accountAssetDetail(accountId, accountName, assetId):
            AccountAssetDetailScreen(accountId: accountId, accountName: accountName, assetId: assetId)
        case let .accountTransactions(accountId):
            ScopedTransactionsScreen(scope: .account(accountId: accountId), title: "Transactions")
        case let .assetTransactions(assetId, accountId, title, subtitle):
            ScopedTransactionsScreen(scope: .asset(assetId: assetId, accountId: accountId), title: title, subtitle: subtitle)
        case .addAccount:
            AddAccountScreen(accountId: nil)
        case let .editAccount(accountId):
            AddAccountScreen(accountId: accountId)
        case let .assetOverview(assetId, userAsset):
            AssetOverviewScreen(assetId: assetId, userAsset: userAsset)
        case .customAssets:
            CustomAssetsScreen()
        case .customCategories:
            CustomCategoriesScreen()
        case .connectors:
            ConnectorsScreen()
        case let .connectorDetail(providerKind):
            ConnectorDetailScreen(providerKind: providerKind)
        case let .connectionDetail(connectionId):
            ConnectionDetailScreen(connectionId: connectionId)
        case let .bindingSetup(connectionId):
            BindingSetupScreen(connectionId: connectionId)
        case let .providerAccountDetail(connectionId, providerAccountId, displayName, currency):
            ProviderAccountDetailScreen(
                connectionId: connectionId,
                providerAccountId: providerAccountId,
                displayName: displayName,
                currency: currency
            )
        case let .connectTrueLayer(code, state, error):
            ConnectTrueLayerScreen(oauthCode: code, oauthState: state, oauthError: error)
        case let .connectEnableBanking(code, state, error):
            ConnectEnableBankingScreen(oauthCode: code, oauthState: state, oauthError: error)
        case .connectTrading212:
            ConnectTrading212Screen()
        }
    }
}
