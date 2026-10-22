import SvertoCore

enum ChatSubmission {
    case message(turn: ChatMessage, text: String, files: [ChatAttachment])
    case toolApproval(callId: String, approved: Bool)
}
