package com.sverto.app.feature.aichat

import android.net.Uri
import uniffi.sverto_core.ChatMessage
import uniffi.sverto_core.ConversationItem

data class AiChatUiState(
    val conversations: List<ConversationItem> = emptyList(),
    val activeConversationId: String? = null,
    val messages: List<ChatMessage> = emptyList(),
    val isStreaming: Boolean = false,
    val inputText: String = "",
    val selectedFileUris: List<Uri> = emptyList(),
    val pendingApprovalCallId: String? = null,
    val error: String? = null,
    val rateLimit: RateLimitInfo? = null,
    val blockedByAiFeatures: ChatRequest? = null,
)

data class RateLimitInfo(
    val message: String,
    val retryAfterSeconds: Long?,
)

sealed interface ChatRequest {
    data class Send(
        val text: String,
        val fileUris: List<Uri>,
    ) : ChatRequest

    data class ToolApproval(
        val callId: String,
        val approved: Boolean,
    ) : ChatRequest
}
