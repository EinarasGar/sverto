import SvertoCore
import SwiftUI

struct ConversationsSheet: View {
    let model: AiChatViewModel

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ConversationsList(model: model) { dismiss() }
                .navigationTitle("Conversations")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { dismiss() }
                    }
                }
        }
        .presentationDetents([.medium, .large])
    }
}

struct ConversationsList: View {
    let model: AiChatViewModel
    let onPick: () -> Void

    @State private var pendingDelete: ConversationItem?
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        List {
            Section {
                Button {
                    model.startNewConversation()
                    onPick()
                } label: {
                    Label("New chat", systemImage: "square.and.pencil")
                }
            }
            if model.conversations.isEmpty {
                ContentUnavailableView(
                    "No conversations yet",
                    systemImage: "bubble.left.and.text.bubble.right",
                    description: Text("Start a new chat to ask Myra about your finances.")
                )
                .listRowBackground(Color.clear)
                .listRowInsets(sizeClass == .regular ? EdgeInsets() : nil)
            } else {
                Section("Recent") {
                    ForEach(model.conversations, id: \.id) { conversation in
                        ConversationRow(conversation: conversation, isSelected: conversation.id == model.activeConversationId) {
                            model.selectConversation(id: conversation.id)
                            onPick()
                        }
                        .swipeActions(edge: .trailing) {
                            Button(role: .destructive) {
                                pendingDelete = conversation
                            } label: {
                                Label("Delete", systemImage: "trash")
                            }
                        }
                        .modifier(deleteDialog(.trigger, for: conversation))
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .refreshable { await model.loadConversations() }
        .modifier(deleteDialog(.screen))
    }

    private func deleteDialog(_ source: DialogSource, for row: ConversationItem? = nil) -> some ViewModifier {
        ConfirmationDialog(
            source: source,
            title: "Delete this conversation?",
            isPresented: Binding(
                get: { pendingDelete != nil && (row == nil || pendingDelete?.id == row?.id) },
                set: { if !$0 { pendingDelete = nil } }
            ),
            item: pendingDelete
        ) { conversation in
            Button("Delete conversation", role: .destructive) {
                model.deleteConversation(id: conversation.id)
            }
        } message: { _ in
            EmptyView()
        }
    }
}

private struct ConversationRow: View {
    let conversation: ConversationItem
    let isSelected: Bool
    let action: () -> Void
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    var body: some View {
        Button(action: action) {
            HStack {
                Text(conversation.title)
                    .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
                Spacer()
                if isSelected {
                    Image(systemName: "checkmark")
                        .foregroundStyle(.tint)
                        .accessibilityHidden(true)
                }
            }
        }
        .foregroundStyle(Color.primary)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
