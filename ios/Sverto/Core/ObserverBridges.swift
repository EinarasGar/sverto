import Foundation
import SvertoCore

// Rust invokes observer callbacks on its own threads while holding a module mutex.
// Each bridge is nonisolated, forwards the payload through a @Sendable closure and never
// calls back into AppStore. Callers hop to the main actor inside the closure.

nonisolated final class PortfolioObserverBridge: PortfolioObserver {
    private let handler: @Sendable (PortfolioState) -> Void
    init(_ handler: @escaping @Sendable (PortfolioState) -> Void) { self.handler = handler }
    func onPortfolioChanged(state: PortfolioState) { handler(state) }
}

nonisolated final class AccountsObserverBridge: AccountsObserver {
    private let handler: @Sendable (AccountsState) -> Void
    init(_ handler: @escaping @Sendable (AccountsState) -> Void) { self.handler = handler }
    func onAccountsChanged(state: AccountsState) { handler(state) }
}

nonisolated final class CategoriesObserverBridge: CategoriesObserver {
    private let handler: @Sendable (CategoriesState) -> Void
    init(_ handler: @escaping @Sendable (CategoriesState) -> Void) { self.handler = handler }
    func onCategoriesChanged(state: CategoriesState) { handler(state) }
}

nonisolated final class AccountDetailObserverBridge: AccountDetailObserver {
    private let handler: @Sendable (AccountDetailState) -> Void
    init(_ handler: @escaping @Sendable (AccountDetailState) -> Void) { self.handler = handler }
    func onAccountDetailChanged(state: AccountDetailState) { handler(state) }
}

nonisolated final class ScopedTransactionsObserverBridge: ScopedTransactionsObserver {
    private let handler: @Sendable (ScopedTransactionsState) -> Void
    init(_ handler: @escaping @Sendable (ScopedTransactionsState) -> Void) { self.handler = handler }
    func onScopedTransactionsChanged(state: ScopedTransactionsState) { handler(state) }
}

nonisolated final class AssetDetailObserverBridge: AssetDetailObserver {
    private let handler: @Sendable (AssetDetailState) -> Void
    init(_ handler: @escaping @Sendable (AssetDetailState) -> Void) { self.handler = handler }
    func onAssetDetailChanged(state: AssetDetailState) { handler(state) }
}

nonisolated final class AssetOverviewObserverBridge: AssetOverviewObserver {
    private let handler: @Sendable (AssetOverviewState) -> Void
    init(_ handler: @escaping @Sendable (AssetOverviewState) -> Void) { self.handler = handler }
    func onAssetOverviewChanged(state: AssetOverviewState) { handler(state) }
}

nonisolated final class TransactionsObserverBridge: TransactionsObserver {
    private let handler: @Sendable (TransactionsState) -> Void
    init(_ handler: @escaping @Sendable (TransactionsState) -> Void) { self.handler = handler }
    func onTransactionsChanged(state: TransactionsState) { handler(state) }
}

nonisolated final class QuickUploadsObserverBridge: QuickUploadsObserver {
    private let handler: @Sendable (QuickUploadsState) -> Void
    init(_ handler: @escaping @Sendable (QuickUploadsState) -> Void) { self.handler = handler }
    func onQuickUploadsChanged(state: QuickUploadsState) { handler(state) }
}

nonisolated final class ConnectionObserverBridge: ConnectionObserver {
    private let handler: @Sendable (ConnectionStatus) -> Void
    init(_ handler: @escaping @Sendable (ConnectionStatus) -> Void) { self.handler = handler }
    func onConnectionStatusChanged(status: ConnectionStatus) { handler(status) }
}

nonisolated final class AuthObserverBridge: AuthObserver {
    private let handler: @Sendable () -> Void
    init(_ handler: @escaping @Sendable () -> Void) { self.handler = handler }
    func onSessionExpired() { handler() }
}

nonisolated final class AiChatObserverBridge: AiChatObserver {
    private let conversations: @Sendable ([ConversationItem]) -> Void
    private let messages: @Sendable (String, [ChatMessage]) -> Void
    private let stream: @Sendable (String, ChatStreamEvent) -> Void

    init(
        conversations: @escaping @Sendable ([ConversationItem]) -> Void,
        messages: @escaping @Sendable (String, [ChatMessage]) -> Void,
        stream: @escaping @Sendable (String, ChatStreamEvent) -> Void
    ) {
        self.conversations = conversations
        self.messages = messages
        self.stream = stream
    }

    func onConversationsChanged(conversations: [ConversationItem]) { self.conversations(conversations) }
    func onMessagesChanged(conversationId: String, messages: [ChatMessage]) { self.messages(conversationId, messages) }
    func onStreamEvent(conversationId: String, event: ChatStreamEvent) { stream(conversationId, event) }
}
