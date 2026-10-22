import Foundation
import Observation
import SvertoCore

@Observable
final class AiChatViewModel {
    private(set) var conversations: [ConversationItem] = []
    private(set) var activeConversationId: String?
    private(set) var messages: [ChatMessage] = []
    private(set) var isStreaming = false
    var inputText = ""
    private(set) var attachments: [ChatAttachment] = []
    private(set) var pendingApprovalCallId: String?
    private(set) var error: String?
    private(set) var rateLimit: RateLimitInfo?
    private(set) var historyState: ChatHistoryState = .idle
    private(set) var isHistoryLoadRunning = false
    let aiConsent: AiConsentGate

    private let store: AppStore
    private var sendTask: Task<Void, Never>?
    private var observer: AiChatObserverBridge?
    private let historyReceipt = ChatHistoryReceipt()
    private var pendingHistoryRequest: ChatHistoryRequest?
    private var historyGeneration: UInt = 0
    private var historyLoadTask: Task<Void, Never>?
    private var lastSubmission: ChatSubmission?

    init(store: AppStore) {
        self.store = store
        aiConsent = AiConsentGate(store: store)
    }

    private func startObserving() {
        guard observer == nil else { return }
        let observer = AiChatObserverBridge(
            conversations: { [weak self] conversations in
                Task { @MainActor in self?.handleConversationsChanged(conversations) }
            },
            messages: { [weak self, historyReceipt] conversationId, messages in
                if historyReceipt.receive(conversationId: conversationId, messages: messages) { return }
                Task { @MainActor in self?.handleMessagesChanged(conversationId: conversationId, messages: messages) }
            },
            stream: { [weak self] conversationId, event in
                Task { @MainActor in self?.applyStreamEvent(event, conversationId: conversationId) }
            }
        )
        self.observer = observer
        // Stream events must keep arriving while another tab is showing.
        store.observeAiChat(observer: observer)
    }

    var canSend: Bool {
        !isHistoryLoadRunning && historyState != .loading && historyState != .failed
            && (!inputText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !attachments.isEmpty)
    }

    var pendingApproval: ToolCallPart? {
        guard pendingApprovalCallId != nil else { return nil }
        return messages.lazy.compactMap(\.pendingApproval).first
    }

    func loadConversations() async {
        startObserving()
        await store.loadConversations()
    }

    func selectConversation(id: String) {
        abandonActiveExchange()
        messages = []
        error = nil
        rateLimit = nil
        activeConversationId = id
        loadHistory(id: id)
    }

    func retryHistory() {
        guard let activeConversationId, historyState == .failed else { return }
        loadHistory(id: activeConversationId)
    }

    func startNewConversation() {
        abandonActiveExchange()
        invalidateHistory()
        activeConversationId = nil
        messages = []
        inputText = ""
        attachments = []
        error = nil
        rateLimit = nil
    }

    func deleteConversation(id: String) {
        abandonActiveExchange()
        invalidateHistory()
        messages = []
        activeConversationId = nil
        Task {
            do {
                try await store.deleteConversation(id: id)
            } catch {
                self.error = error.friendlyMessage
            }
        }
    }

    func addAttachments(_ items: [ChatAttachment]) {
        guard !items.isEmpty else { return }
        attachments.append(contentsOf: items)
    }

    func removeAttachment(_ item: ChatAttachment) {
        attachments.removeAll { $0.id == item.id }
    }

    func sendMessage() {
        guard !isStreaming, canSend else { return }
        aiConsent.perform { [weak self] in self?.submitMessage() }
    }

    func sendSuggestion(_ suggestion: String) {
        aiConsent.perform { [weak self] in
            guard let self else { return }
            inputText = suggestion
            submitMessage()
        }
    }

    private func submitMessage() {
        let text = inputText.trimmingCharacters(in: .whitespacesAndNewlines)
        let files = attachments
        guard !isStreaming, canSend else { return }
        var parts = files.map { MessagePart.file(fileId: "", mediaType: $0.mimeType, url: $0.localURL.absoluteString) }
        if !text.isEmpty {
            parts.append(.text(content: text))
        }
        let userTurn = ChatMessage(role: "user", parts: parts)
        let conversationId = activeConversationId
        messages.append(userTurn)
        inputText = ""
        attachments = []
        isStreaming = true
        error = nil
        rateLimit = nil
        lastSubmission = .message(turn: userTurn, text: text, files: files)
        sendTask = Task {
            let targetId: String
            if let conversationId {
                targetId = conversationId
            } else {
                do {
                    targetId = try await store.createConversation()
                } catch {
                    restoreUnsentTurn(userTurn, text: text, files: files)
                    self.error = "Failed to create conversation: \(error.friendlyMessage)"
                    return
                }
            }
            guard !Task.isCancelled else {
                restoreUnsentTurn(userTurn, text: text, files: files)
                return
            }
            activeConversationId = targetId
            var fileIds: [String] = []
            for file in files {
                if let fileId = try? await store.uploadChatFile(imageData: file.data, mimeType: file.mimeType, fileName: file.fileName) {
                    fileIds.append(fileId)
                }
            }
            guard !Task.isCancelled else {
                restoreUnsentTurn(userTurn, text: text, files: files)
                return
            }
            await store.sendMessage(conversationId: targetId, text: text, fileIds: fileIds)
        }
    }

    func approveTool(approved: Bool) {
        guard pendingApprovalCallId != nil else { return }
        aiConsent.perform { [weak self] in self?.submitToolApproval(approved: approved) }
    }

    private func submitToolApproval(approved: Bool) {
        guard let conversationId = activeConversationId, let callId = pendingApprovalCallId else { return }
        setToolState(approved ? .inputAvailable : .outputDenied, callId: callId)
        isStreaming = true
        pendingApprovalCallId = nil
        lastSubmission = .toolApproval(callId: callId, approved: approved)
        Task { await store.approveTool(conversationId: conversationId, callId: callId, approved: approved) }
    }

    private func setToolState(_ state: ToolCallState, callId: String) {
        messages = messages.map { message in
            guard message.isAssistant else { return message }
            let parts = message.parts.map { part -> MessagePart in
                guard var tool = part.toolCall, tool.callId == callId else { return part }
                tool.state = state
                return tool.messagePart
            }
            return ChatMessage(role: message.role, parts: parts)
        }
    }

    func cancelStream() {
        abandonActiveExchange()
    }

    func clearError() {
        error = nil
    }

    func clearRateLimit() {
        rateLimit = nil
    }

    private func loadHistory(id: String) {
        startObserving()
        historyGeneration &+= 1
        historyState = .loading
        pendingHistoryRequest = ChatHistoryRequest(conversationId: id, generation: historyGeneration)
        guard historyLoadTask == nil else { return }
        historyLoadTask = Task {
            isHistoryLoadRunning = true
            // The core changes its active conversation on completion, so history reads must drain before sending.
            while let request = pendingHistoryRequest {
                pendingHistoryRequest = nil
                historyReceipt.begin(conversationId: request.conversationId)
                await store.loadMessages(conversationId: request.conversationId)
                let loadedMessages = historyReceipt.finish()
                guard request.generation == historyGeneration, activeConversationId == request.conversationId else { continue }
                if let loadedMessages {
                    handleMessagesChanged(conversationId: request.conversationId, messages: loadedMessages)
                    historyState = .loaded
                } else {
                    historyState = .failed
                }
            }
            isHistoryLoadRunning = false
            historyLoadTask = nil
        }
    }

    private func invalidateHistory() {
        historyGeneration &+= 1
        pendingHistoryRequest = nil
        historyState = .idle
    }

    private func abandonActiveExchange() {
        sendTask?.cancel()
        sendTask = nil
        store.cancelStream()
        isStreaming = false
        pendingApprovalCallId = nil
        lastSubmission = nil
    }

    private func reopenRejectedSubmission() {
        guard let submission = lastSubmission else { return }
        lastSubmission = nil
        switch submission {
        case let .message(turn, text, files):
            restoreUnsentTurn(turn, text: text, files: files)
            aiConsent.requestConsent { [weak self] in self?.submitMessage() }
        case let .toolApproval(callId, approved):
            setToolState(.approvalRequested, callId: callId)
            pendingApprovalCallId = callId
            aiConsent.requestConsent { [weak self] in self?.submitToolApproval(approved: approved) }
        }
    }

    private func restoreUnsentTurn(_ turn: ChatMessage, text: String, files: [ChatAttachment]) {
        guard let index = messages.lastIndex(of: turn) else { return }
        messages.remove(at: index)
        inputText = text
        attachments = files
        isStreaming = false
    }

    private func handleConversationsChanged(_ conversations: [ConversationItem]) {
        self.conversations = conversations.sorted { lhs, rhs in
            Self.updatedDate(lhs) > Self.updatedDate(rhs)
        }
    }

    private static func updatedDate(_ conversation: ConversationItem) -> Date {
        DateText.parseServerTimestamp(conversation.updatedAt) ?? .distantPast
    }

    private func handleMessagesChanged(conversationId: String, messages: [ChatMessage]) {
        guard conversationId == activeConversationId else { return }
        self.messages = messages
        pendingApprovalCallId = messages.lazy.compactMap(\.pendingApproval).first?.callId
        error = nil
    }

    private func applyStreamEvent(_ event: ChatStreamEvent, conversationId: String) {
        guard conversationId == activeConversationId else { return }
        var updated = messages
        let continuesAssistantTurn = updated.last?.isAssistant == true
        var parts = continuesAssistantTurn ? (updated.last?.parts ?? []) : []

        func commit() {
            let turn = ChatMessage(role: "assistant", parts: parts)
            if continuesAssistantTurn {
                updated[updated.count - 1] = turn
            } else {
                updated.append(turn)
            }
            messages = updated
        }

        switch event {
        case let .textDelta(delta):
            if let existing = parts.last?.textContent {
                parts[parts.count - 1] = .text(content: existing + delta)
            } else {
                parts.append(.text(content: delta))
            }
        case let .reasoningDelta(delta):
            if let existing = parts.last?.reasoningContent {
                parts[parts.count - 1] = .reasoning(content: existing + delta)
            } else {
                parts.append(.reasoning(content: delta))
            }
        case let .toolCall(callId, name, params):
            parts.append(ToolCallPart(callId: callId, name: name, params: params, state: .inputAvailable, output: nil).messagePart)
        case let .toolResult(_, output):
            if let index = parts.lastIndex(where: { part in
                guard let tool = part.toolCall else { return false }
                return !tool.isAwaitingApproval && tool.output == nil
            }), var tool = parts[index].toolCall {
                tool.state = .outputAvailable
                tool.output = output
                parts[index] = tool.messagePart
            }
        case let .toolApprovalRequired(callId, _, _):
            if let index = parts.lastIndex(where: { $0.toolCall?.callId == callId }), var tool = parts[index].toolCall {
                tool.state = .approvalRequested
                tool.output = nil
                parts[index] = tool.messagePart
            }
            commit()
            pendingApprovalCallId = callId
            isStreaming = false
            return
        case let .error(message):
            error = message
            return
        case let .rateLimited(message, retryAfterSeconds):
            isStreaming = false
            rateLimit = RateLimitInfo(message: message, retryAfterSeconds: retryAfterSeconds)
            return
        case .aiFeaturesDisabled:
            isStreaming = false
            reopenRejectedSubmission()
            return
        case .done:
            isStreaming = false
            return
        }
        commit()
    }
}
