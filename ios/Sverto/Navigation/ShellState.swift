import Foundation
import Observation
import SvertoCore

nonisolated struct TransactionDetailState: Hashable {
    var transaction: TransactionListItem
    var isInGroup: Bool
}

nonisolated struct GroupTransactionItem: Hashable {
    var input: CreateTransactionInput
    var descriptionDisplay: String
    var typeLabel: String
    var amountDisplay: String
    var accountName: String
    var assetDisplay: String
    var categoryName: String = ""
}

nonisolated struct PendingBindingChoice: Equatable {
    var connectionId: String
    var providerAccountId: String
    var account: AccountListItem
}

@Observable
final class ShellState {
    var selectedTab: AppTab = .portfolio
    var portfolioPath: [AppRoute] = []
    var transactionsPath: [AppRoute] = []
    var accountsPath: [AppRoute] = []
    var chatPath: [AppRoute] = []

    var isSettingsPresented = false
    var settingsPath: [AppRoute] = []

    var transactionCache: [String: TransactionDetailState] = [:]
    var pendingGroupTransaction: GroupTransactionItem?
    var editingGroupIndex: Int?
    var pendingGroupSelection: [String] = []
    var groupChildDrafts: [GroupTransactionItem] = []

    var pendingSharedFiles: [PickResult] = []
    var pendingOAuthReturn: AppRoute?
    var pendingBindingChoice: PendingBindingChoice?

    var refreshRequests = 0

    func path(for tab: AppTab) -> [AppRoute] {
        switch tab {
        case .portfolio: portfolioPath
        case .transactions: transactionsPath
        case .accounts: accountsPath
        case .chat: chatPath
        }
    }

    func setPath(_ path: [AppRoute], for tab: AppTab) {
        switch tab {
        case .portfolio: portfolioPath = path
        case .transactions: transactionsPath = path
        case .accounts: accountsPath = path
        case .chat: chatPath = path
        }
    }

    func push(_ route: AppRoute) {
        if isSettingsPresented {
            settingsPath.append(route)
        } else {
            var path = path(for: selectedTab)
            path.append(route)
            setPath(path, for: selectedTab)
        }
    }

    func pop() {
        if isSettingsPresented {
            if !settingsPath.isEmpty { settingsPath.removeLast() }
        } else {
            var path = path(for: selectedTab)
            if !path.isEmpty { path.removeLast() }
            setPath(path, for: selectedTab)
        }
    }

    func showTransactionDetail(_ transaction: TransactionListItem, isInGroup: Bool) {
        transactionCache[transaction.id] = TransactionDetailState(transaction: transaction, isInGroup: isInGroup)
        push(.transactionDetail(id: transaction.id))
    }

    // At compact width the path is empty whenever a tab root is visible, so this equals push there;
    // beside a list column it swaps the detail instead of stacking a second one.
    func showFromRoot(_ route: AppRoute) {
        setPath([route], for: selectedTab)
    }

    func showTransactionDetailFromRoot(_ transaction: TransactionListItem) {
        transactionCache[transaction.id] = TransactionDetailState(transaction: transaction, isInGroup: false)
        showFromRoot(.transactionDetail(id: transaction.id))
    }

    func presentSettings(_ path: [AppRoute] = []) {
        settingsPath = path
        isSettingsPresented = true
    }

    func dismissSettings() {
        isSettingsPresented = false
        settingsPath.removeAll()
    }

    func select(_ tab: AppTab) {
        if selectedTab == tab {
            setPath([], for: tab)
        } else {
            selectedTab = tab
        }
    }

    func reset() {
        selectedTab = .portfolio
        portfolioPath = []
        transactionsPath = []
        accountsPath = []
        chatPath = []
        isSettingsPresented = false
        settingsPath = []
        transactionCache = [:]
        pendingGroupTransaction = nil
        editingGroupIndex = nil
        pendingGroupSelection = []
        groupChildDrafts = []
        pendingSharedFiles = []
        pendingOAuthReturn = nil
        pendingBindingChoice = nil
    }
}
