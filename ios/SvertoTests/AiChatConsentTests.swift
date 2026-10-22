import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct AiChatConsentTests {
    @Test func notNowKeepsTheDraftAndSendsNothing() {
        let store = ConsentChatStore(noHandle: .init())
        let model = AiChatViewModel(store: store)
        model.inputText = "How much did I spend?"

        model.sendMessage()
        model.aiConsent.decline()
        model.aiConsent.sheetDismissed()

        #expect(model.inputText == "How much did I spend?")
        #expect(model.messages.isEmpty)
        #expect(!model.isStreaming)
        #expect(store.sentTexts.isEmpty)
    }

    @Test func turningOnSendsTheDraft() async throws {
        let store = ConsentChatStore(noHandle: .init())
        let model = AiChatViewModel(store: store)
        model.inputText = "Hi"
        model.sendMessage()
        #expect(model.aiConsent.isPresented)

        await model.aiConsent.turnOn()
        model.aiConsent.sheetDismissed()

        try await waitUntil { store.sentTexts == ["Hi"] }
        #expect(model.inputText.isEmpty)
        #expect(model.messages.count == 1)
    }

    @Test func declinedSuggestionLeavesTheTypedDraftAlone() {
        let store = ConsentChatStore(noHandle: .init())
        let model = AiChatViewModel(store: store)
        model.inputText = "My own question"

        model.sendSuggestion("What's my spending this month?")
        model.aiConsent.decline()
        model.aiConsent.sheetDismissed()

        #expect(model.inputText == "My own question")
        #expect(store.sentTexts.isEmpty)
    }

    @Test func serverRejectionRestoresTheMessageAndResendsAfterConsent() async throws {
        let store = ConsentChatStore(noHandle: .init())
        store.setEnabled(true)
        let model = AiChatViewModel(store: store)
        await model.loadConversations()
        model.inputText = "Hi"
        model.sendMessage()
        try await waitUntil { store.sentTexts == ["Hi"] }

        store.setEnabled(false)
        store.currentObserver?.onStreamEvent(conversationId: ConsentChatStore.conversationId, event: .aiFeaturesDisabled)
        store.currentObserver?.onStreamEvent(conversationId: ConsentChatStore.conversationId, event: .done)
        try await waitUntil { model.aiConsent.isPresented && !model.isStreaming }

        #expect(model.messages.isEmpty)
        #expect(model.inputText == "Hi")
        await model.aiConsent.turnOn()
        model.aiConsent.sheetDismissed()
        try await waitUntil { store.sentTexts == ["Hi", "Hi"] }
        #expect(model.inputText.isEmpty)
    }

    @Test func approvalWhileDisabledAsksFirstAndKeepsTheApprovalCard() async throws {
        let store = ConsentChatStore(noHandle: .init())
        store.setEnabled(true)
        let model = AiChatViewModel(store: store)
        try await requestToolApproval(model: model, store: store)
        store.setEnabled(false)

        model.approveTool(approved: false)

        #expect(model.aiConsent.isPresented)
        #expect(model.pendingApproval?.callId == "call")
        #expect(store.approvalDecisions.isEmpty)
    }

    @Test func serverRejectionOfAnApprovalRestoresTheCardAndRetriesAfterConsent() async throws {
        let store = ConsentChatStore(noHandle: .init())
        store.setEnabled(true)
        let model = AiChatViewModel(store: store)
        try await requestToolApproval(model: model, store: store)

        model.approveTool(approved: true)
        try await waitUntil { store.approvalDecisions == [true] }
        store.setEnabled(false)
        store.currentObserver?.onStreamEvent(conversationId: ConsentChatStore.conversationId, event: .aiFeaturesDisabled)
        try await waitUntil { model.aiConsent.isPresented }

        #expect(model.pendingApproval?.callId == "call")
        #expect(!model.isStreaming)
        await model.aiConsent.turnOn()
        model.aiConsent.sheetDismissed()
        try await waitUntil { store.approvalDecisions == [true, true] }
    }

    private func requestToolApproval(model: AiChatViewModel, store: ConsentChatStore) async throws {
        await model.loadConversations()
        model.inputText = "Add a coffee"
        model.sendMessage()
        try await waitUntil { store.sentTexts == ["Add a coffee"] }
        store.currentObserver?.onStreamEvent(conversationId: ConsentChatStore.conversationId, event: .toolCall(callId: "call", name: "create_transaction", params: "{}"))
        store.currentObserver?.onStreamEvent(conversationId: ConsentChatStore.conversationId, event: .toolApprovalRequired(callId: "call", name: "create_transaction", params: "{}"))
        try await waitUntil { model.pendingApproval != nil }
    }

    private func waitUntil(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while !condition(), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(condition())
    }
}

nonisolated private final class ConsentChatStore: AppStore, @unchecked Sendable {
    static let conversationId = "conversation"

    private let lock = NSLock()
    private var enabled = false
    private var observer: (any AiChatObserver)?
    private var sent: [String] = []
    private var approvals: [Bool] = []

    var currentObserver: (any AiChatObserver)? { lock.withLock { observer } }
    var sentTexts: [String] { lock.withLock { sent } }
    var approvalDecisions: [Bool] { lock.withLock { approvals } }

    func setEnabled(_ enabled: Bool) {
        lock.withLock { self.enabled = enabled }
    }

    override func getAiFeaturesEnabled() -> Bool {
        lock.withLock { enabled }
    }

    override func setAiFeaturesEnabled(enabled: Bool) async throws {
        lock.withLock { self.enabled = enabled }
    }

    override func observeAiChat(observer: any AiChatObserver) {
        lock.withLock { self.observer = observer }
    }

    override func unobserveAiChat() {
        lock.withLock { observer = nil }
    }

    override func createConversation() async throws -> String {
        Self.conversationId
    }

    override func sendMessage(conversationId: String, text: String, fileIds: [String]) async {
        lock.withLock { sent.append(text) }
    }

    override func approveTool(conversationId: String, callId: String, approved: Bool) async {
        lock.withLock { approvals.append(approved) }
    }

    override func loadConversations() async {}
    override func loadMessages(conversationId: String) async {}
    override func cancelStream() {}
}
