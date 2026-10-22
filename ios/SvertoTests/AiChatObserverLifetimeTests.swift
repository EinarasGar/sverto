import Foundation
import SwiftUI
import SvertoCore
import Testing
import UIKit
@testable import Sverto

@MainActor
struct AiChatObserverLifetimeTests {
    @Test func discardedInitialModelDoesNotTakeConversationOrStreamUpdates() async throws {
        let store = ChatObserverStore(noHandle: .init())
        let model = AiChatViewModel(store: store)
        await model.loadConversations()
        model.selectConversation(id: "conversation")
        store.currentObserver?.onMessagesChanged(conversationId: "conversation", messages: [ChatMessage(role: "assistant", parts: [.text(content: "Before")])])
        let initialDeadline = ContinuousClock.now + .seconds(2)
        while model.messages.isEmpty, ContinuousClock.now < initialDeadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(model.messages.last?.parts.last?.textContent == "Before")

        _ = AiChatViewModel(store: store)
        store.currentObserver?.onConversationsChanged(conversations: [ConversationItem(id: "conversation", title: "Retained conversation", createdAt: "2026-09-29T12:00:00Z", updatedAt: "2026-09-29T12:00:00Z")])
        store.currentObserver?.onStreamEvent(conversationId: "conversation", event: .textDelta(delta: " after"))
        let updateDeadline = ContinuousClock.now + .seconds(2)
        while model.conversations.isEmpty, ContinuousClock.now < updateDeadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(model.conversations.first?.title == "Retained conversation")
        #expect(model.messages.last?.parts.last?.textContent == "Before after")
    }

    @Test func screenReconstructionAndTabHidingKeepTheSameObserver() async throws {
        let store = ChatObserverStore(noHandle: .init())
        let environment = AppEnvironment(flavor: .dev, apiBaseURL: try #require(URL(string: "http://127.0.0.1:1")), clerkPublishableKey: "")
        func services() -> AppServices {
            AppServices(environment: environment, store: store, connectivity: ConnectivityMonitor(store: store, probe: ConnectivityProbe()), clerk: ClerkGateway(isConfigured: false))
        }
        func root(services: AppServices, selection: Int) -> some View {
            TabView(selection: .constant(selection)) {
                Tab("Chat", systemImage: "message", value: 0) { AiChatScreen() }
                Tab("Other", systemImage: "house", value: 1) { Text("Other tab") }
            }
            .environment(services)
            .environment(services.shell)
            .environment(ToastCenter())
        }
        let host = UIHostingController(rootView: root(services: services(), selection: 0))
        let scene = try #require(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        host.view.layoutIfNeeded()
        let initialDeadline = ContinuousClock.now + .seconds(2)
        while store.currentObserver == nil, ContinuousClock.now < initialDeadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        let observer = try #require(store.currentObserver)

        host.rootView = root(services: services(), selection: 0)
        host.view.layoutIfNeeded()
        await Task.yield()
        #expect(store.currentObserver === observer)

        host.rootView = root(services: services(), selection: 1)
        host.view.layoutIfNeeded()
        await Task.yield()
        #expect(store.currentObserver === observer)
    }
}

nonisolated private final class ChatObserverStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var observer: (any AiChatObserver)?

    var currentObserver: (any AiChatObserver)? { lock.withLock { observer } }

    override func observeAiChat(observer: any AiChatObserver) {
        lock.withLock { self.observer = observer }
    }

    override func unobserveAiChat() {
        lock.withLock { observer = nil }
    }

    override func loadConversations() async {}
    override func loadMessages(conversationId: String) async {}
    override func cancelStream() {}
}
