package com.sverto.app.feature.transactions.quickupload

import uniffi.sverto_core.TransactionCandidate

sealed interface PreparedQuickUpload {
    class Ready(
        val data: ByteArray,
        val thumbnail: ByteArray,
        val mimeType: String,
    ) : PreparedQuickUpload

    data class Rejected(
        val message: String,
    ) : PreparedQuickUpload
}

data class ProposalSummary(
    val description: String,
    val date: String,
    val amount: String?,
    val currency: String? = null,
)

enum class QuickUploadStatus {
    QUEUED,
    UPLOADING,
    PROCESSING,
    READY,
    FAILED,
}

data class QuickUploadUiItem(
    val id: String,
    val isLocal: Boolean,
    val status: QuickUploadStatus,
    val thumbnailBytes: ByteArray?,
    val proposalType: String?,
    val proposalSummary: ProposalSummary?,
    val errorMessage: String?,
    val createdAt: Long,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is QuickUploadUiItem) return false
        return id == other.id && status == other.status && proposalSummary == other.proposalSummary && errorMessage == other.errorMessage
    }

    override fun hashCode(): Int = id.hashCode()
}

data class LedgerMatches(
    val candidates: List<TransactionCandidate> = emptyList(),
    val selectedId: String? = null,
    val merge: Boolean = true,
) {
    val selected: TransactionCandidate? get() = candidates.firstOrNull { it.id == selectedId }

    val mergeTargetId: String? get() = selected?.id?.takeIf { merge }

    fun replacedBy(fresh: List<TransactionCandidate>): LedgerMatches =
        copy(
            candidates = fresh,
            selectedId = selectedId?.takeIf { id -> fresh.any { it.id == id } } ?: fresh.firstOrNull()?.id,
        )

    fun selectingNext(): LedgerMatches {
        if (candidates.size < 2) return this
        val next = (candidates.indexOfFirst { it.id == selectedId } + 1) % candidates.size
        return copy(selectedId = candidates[next].id)
    }
}
