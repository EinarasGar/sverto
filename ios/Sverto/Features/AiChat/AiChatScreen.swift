import SvertoCore
import SwiftUI

struct AiChatScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        AiChatContent(store: services.store)
    }
}

private struct AiChatContent: View {
    @State private var model: AiChatViewModel
    @State private var isConversationsPresented = false
    @State private var width = CGFloat.infinity
    @State private var fold: CGRect?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.horizontalSizeClass) private var sizeClass

    init(store: AppStore) {
        _model = State(initialValue: AiChatViewModel(store: store))
    }

    private var showsConversationsColumn: Bool {
        sizeClass == .regular && width >= Layout.minListDetailWidth
    }

    var body: some View {
        HStack(spacing: 0) {
            if showsConversationsColumn {
                ConversationsList(model: model) {}
                    .frame(width: fold?.minX ?? min(max((width * 0.3).rounded(), 300), 360))
                ColumnSeparator(fold: fold)
            }
            thread
                .maxContentWidth(Layout.readableWidth)
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width.rounded() } action: { width = $0 }
        .onVerticalFoldChange { fold = $0 }
        .onChange(of: showsConversationsColumn) { _, shows in
            if shows { isConversationsPresented = false }
        }
    }

    private var thread: some View {
        Group {
            if model.historyState == .loading {
                ProgressView("Loading conversation…")
            } else if model.historyState == .failed {
                ContentUnavailableView {
                    Label("Couldn't load conversation", systemImage: "exclamationmark.triangle")
                } description: {
                    Text("Try loading the messages again.")
                } actions: {
                    Button("Retry", action: model.retryHistory)
                }
            } else if model.messages.isEmpty, !model.isStreaming {
                SuggestionChips(onSuggestion: model.sendSuggestion)
                    .disabled(model.isHistoryLoadRunning)
            } else {
                ChatThreadView(model: model)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.screenBackground)
        .safeAreaBar(edge: .bottom, spacing: 0) {
            VStack(spacing: 8) {
                if let error = model.error {
                    HStack(alignment: .top, spacing: 8) {
                        Label(error, systemImage: "exclamationmark.triangle")
                            .font(.footnote)
                            .foregroundStyle(.red)
                            .fixedSize(horizontal: false, vertical: true)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        Button(action: model.clearError) {
                            Label("Dismiss error", systemImage: "xmark")
                                .labelStyle(.iconOnly)
                                .frame(minWidth: 44, minHeight: 44)
                                .contentShape(.rect)
                        }
                        .tint(.primary)
                    }
                }
                if let rateLimit = model.rateLimit {
                    RateLimitBanner(info: rateLimit) { model.clearRateLimit() }
                        .transition(reduceMotion ? .opacity : .move(edge: .bottom).combined(with: .opacity))
                }
                MessageComposer(model: model)
            }
            .padding(.horizontal, 12)
            .padding(.top, 8)
            .padding(.bottom, 8)
            .animation(reduceMotion ? nil : .snappy, value: model.rateLimit)
        }
        .navigationTitle("Myra")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if !showsConversationsColumn {
                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        isConversationsPresented = true
                    } label: {
                        ToolbarSymbol("Conversations", systemImage: "sidebar.left")
                    }
                    .accessibilityLabel("Conversations")
                    .tint(.primary)
                }
            }
            ProfileToolbar()
        }
        .sheet(isPresented: $isConversationsPresented) {
            ConversationsSheet(model: model)
        }
        .aiConsentSheet(model.aiConsent)
        .task { await model.loadConversations() }
        .onChange(of: model.error) { _, error in
            guard let error else { return }
            AccessibilityNotification.Announcement(error).post()
        }
        .onChange(of: model.historyState) { _, state in
            if state == .failed {
                AccessibilityNotification.Announcement("Couldn't load conversation. Retry is available.").post()
            }
        }
    }
}
