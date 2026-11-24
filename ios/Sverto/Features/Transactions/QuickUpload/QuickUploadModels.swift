import Foundation

nonisolated struct PreparedQuickUpload: Sendable {
    let data: Data
    let thumbnail: Data
    let mimeType: String
}

enum QuickUploadOrigin {
    case photos
    case camera
    case files
}

nonisolated enum ProposalType: String, Hashable {
    case transaction
    case transactionGroup = "transaction_group"
}

nonisolated enum QuickUploadStatus: Equatable {
    case queued
    case uploading
    case processing
    case ready
    case failed

    nonisolated init(serverStatus: String) {
        switch serverStatus {
        case "queued": self = .queued
        case "uploading": self = .uploading
        case "pending", "created", "processing": self = .processing
        case "proposal_ready": self = .ready
        case "failed": self = .failed
        default: self = .processing
        }
    }
}

nonisolated struct QuickUploadUiItem: Identifiable, Equatable {
    let id: String
    let status: QuickUploadStatus
    let thumbnail: Data?
    let proposalType: ProposalType?
    let proposalSummary: ProposalSummary?
    let errorMessage: String?
    let createdAt: Int64

    // Thumbnail bytes are excluded on purpose, mirroring Android, so re-emitted rows do not churn the list.
    nonisolated static func == (lhs: QuickUploadUiItem, rhs: QuickUploadUiItem) -> Bool {
        lhs.id == rhs.id
            && lhs.status == rhs.status
            && lhs.proposalSummary == rhs.proposalSummary
            && lhs.errorMessage == rhs.errorMessage
    }
}
