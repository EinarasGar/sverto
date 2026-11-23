package com.sverto.app.feature.imports

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import com.sverto.app.feature.imports.review.NEW_TRANSACTION_CHOICE
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sverto_core.AccountItem
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.ImportAnswer
import uniffi.sverto_core.ImportAssetCandidate
import uniffi.sverto_core.ImportClassificationStatus
import uniffi.sverto_core.ImportDateOrder
import uniffi.sverto_core.ImportFileStatus
import uniffi.sverto_core.ImportItemVerb
import uniffi.sverto_core.ImportItems
import uniffi.sverto_core.ImportPendingRow
import uniffi.sverto_core.ImportSessionDetail
import uniffi.sverto_core.ImportSessionStatus
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

private const val POLL_INTERVAL_MS = 3_000L

data class ImportSessionUiState(
    val detail: ImportSessionDetail? = null,
    val items: ImportItems? = null,
    val pending: List<ImportPendingRow> = emptyList(),
    val accounts: List<AccountItem>? = null,
    val accountChoices: Map<String, String> = emptyMap(),
    val separateAccounts: Set<String> = emptySet(),
    val assetChoices: Map<String, ImportAssetCandidate> = emptyMap(),
    val duplicateChoices: Map<String, String> = emptyMap(),
    val seenDuplicates: Set<String> = emptySet(),
    val applied: AppliedCounts? = null,
    val uploads: List<LocalUpload> = emptyList(),
    val loadError: String? = null,
    val writes: Int = 0,
    val applying: Boolean = false,
) {
    val working: Boolean get() = writes > 0 || applying
}

sealed interface ImportSessionEvent {
    data class Message(
        val text: String,
    ) : ImportSessionEvent

    class AiBlocked(
        val retry: () -> Unit,
    ) : ImportSessionEvent

    data object Deleted : ImportSessionEvent
}

private fun isBusy(detail: ImportSessionDetail): Boolean {
    val classification = detail.summary.classificationStatus
    return detail.files.any { it.status == ImportFileStatus.PENDING || it.status == ImportFileStatus.PROCESSING } ||
        classification == ImportClassificationStatus.WAITING ||
        classification == ImportClassificationStatus.RUNNING ||
        (
            detail.summary.status == ImportSessionStatus.RECONCILING &&
                classification == ImportClassificationStatus.NOT_STARTED
        )
}

class ImportSessionViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(ImportSessionUiState())
    val state: StateFlow<ImportSessionUiState> = _state.asStateFlow()

    private val _events = Channel<ImportSessionEvent>(Channel.BUFFERED)
    val events: Flow<ImportSessionEvent> = _events.receiveAsFlow()

    private var sessionId = ""
    private var resumed = false
    private var pollJob: Job? = null
    private val generation = AtomicLong()
    private val pickedFiles = ConcurrentHashMap<String, PickedFile>()
    private var createAccountFor: String? = null
    private var accountIdsBeforeCreate: Set<String> = emptySet()

    fun resume(id: String) {
        sessionId = id
        resumed = true
        val createdFor = createAccountFor
        createAccountFor = null
        viewModelScope.launch(Dispatchers.IO) { loadAccounts(createdFor) }
        poll()
    }

    fun pause() {
        resumed = false
        pollJob?.cancel()
    }

    private fun poll() {
        if (!resumed) return
        pollJob?.cancel()
        pollJob =
            viewModelScope.launch(Dispatchers.IO) {
                while (true) {
                    val busy =
                        try {
                            load(generation.get())
                        } catch (e: ApiException) {
                            reportLoadFailure(e)
                            false
                        }
                    if (busy != true) break
                    delay(POLL_INTERVAL_MS)
                }
            }
    }

    private suspend fun reportLoadFailure(e: ApiException) {
        if (_state.value.detail == null) {
            _state.update { it.copy(loadError = e.friendlyMessage()) }
        } else {
            _events.send(ImportSessionEvent.Message(e.friendlyMessage()))
        }
    }

    private suspend fun load(startedAt: Long): Boolean? {
        val detail = store.getImportSession(sessionId)
        val classified = detail.summary.classificationStatus == ImportClassificationStatus.CLASSIFIED
        val items = if (classified) store.listImportItems(sessionId) else null
        val pending = if (detail.counts.pending > 0) store.listImportPending(sessionId) else emptyList()
        if (generation.get() != startedAt) return null
        val attached = detail.files.map { it.fileId }.toSet()
        _state.update { state ->
            state.copy(
                detail = detail,
                items = items,
                pending = pending,
                seenDuplicates =
                    state.seenDuplicates +
                        items
                            ?.rows
                            .orEmpty()
                            .filter { it.nearMiss.isNotEmpty() }
                            .map { it.id },
                loadError = null,
                uploads = state.uploads.filterNot { upload -> upload.attachedFileId?.let { it in attached } == true },
            )
        }
        return isBusy(detail)
    }

    private suspend fun refresh() {
        try {
            if (load(generation.incrementAndGet()) == true) poll()
        } catch (e: ApiException) {
            reportLoadFailure(e)
        }
    }

    private suspend fun loadAccounts(createdFor: String?) {
        val accounts =
            try {
                store.getAccountsList()
            } catch (_: ApiException) {
                return
            }
        val created = createdFor?.let { accounts.firstOrNull { it.id !in accountIdsBeforeCreate } }
        _state.update { state ->
            state.copy(
                accounts = accounts,
                accountChoices =
                    if (createdFor != null && created != null) {
                        state.accountChoices + (createdFor to created.id)
                    } else {
                        state.accountChoices
                    },
            )
        }
    }

    private fun write(
        retry: (() -> Unit)? = null,
        block: suspend () -> Unit,
    ) {
        viewModelScope.launch(Dispatchers.IO) {
            pollJob?.cancel()
            generation.incrementAndGet()
            _state.update { it.copy(writes = it.writes + 1) }
            try {
                block()
            } catch (e: ApiException.AiFeaturesDisabled) {
                _events.send(
                    if (retry != null) ImportSessionEvent.AiBlocked(retry) else ImportSessionEvent.Message(e.friendlyMessage()),
                )
            } catch (e: ApiException) {
                _events.send(ImportSessionEvent.Message(e.friendlyMessage()))
            }
            try {
                refresh()
            } finally {
                _state.update { it.copy(writes = it.writes - 1) }
            }
        }
    }

    fun chooseAccount(
        rawAccount: String,
        accountId: String,
    ) = _state.update { it.copy(accountChoices = it.accountChoices + (rawAccount to accountId)) }

    fun setSameAccount(
        rawAccount: String,
        same: Boolean,
    ) = _state.update {
        it.copy(separateAccounts = if (same) it.separateAccounts - rawAccount else it.separateAccounts + rawAccount)
    }

    fun createAccount(
        rawAccount: String,
        navigate: () -> Unit,
    ) {
        viewModelScope.launch(Dispatchers.IO) {
            val known =
                _state.value.accounts ?: try {
                    store.getAccountsList().also { accounts -> _state.update { it.copy(accounts = accounts) } }
                } catch (e: ApiException) {
                    _events.send(ImportSessionEvent.Message(e.friendlyMessage()))
                    return@launch
                }
            accountIdsBeforeCreate = known.map { it.id }.toSet()
            createAccountFor = rawAccount
            withContext(Dispatchers.Main) { navigate() }
        }
    }

    fun answerAccounts(
        askKeys: List<String>,
        accountId: String?,
    ) = write {
        val answer = accountId?.let { ImportAnswer.AccountMap(it) } ?: ImportAnswer.AccountExclude
        askKeys.forEach { store.answerImportAsk(sessionId, it, answer) }
    }

    fun chooseAsset(
        askKey: String,
        asset: ImportAssetCandidate,
    ) = _state.update { it.copy(assetChoices = it.assetChoices + (askKey to asset)) }

    fun answerAsset(
        askKey: String,
        assetId: Int?,
    ) = write {
        store.answerImportAsk(sessionId, askKey, assetId?.let { ImportAnswer.AssetMap(it) } ?: ImportAnswer.AssetExclude)
    }

    fun chooseDuplicate(
        rowId: String,
        choice: String,
    ) = _state.update { it.copy(duplicateChoices = it.duplicateChoices + (rowId to choice)) }

    fun confirmDuplicate(rowId: String) {
        val choice = _state.value.duplicateChoices[rowId] ?: return
        if (choice == NEW_TRANSACTION_CHOICE) {
            setVerb(listOf(rowId), ImportItemVerb.ADD)
        } else {
            setVerb(listOf(rowId), ImportItemVerb.SKIP, choice)
        }
    }

    fun enterBalance(
        askKey: String,
        quantity: String,
    ) = write { store.answerImportAsk(sessionId, askKey, ImportAnswer.EnterBalance(quantity)) }

    fun answerUnlisted(
        askKey: String,
        delete: Boolean,
    ) = write {
        store.answerImportAsk(sessionId, askKey, if (delete) ImportAnswer.DeleteUnlisted else ImportAnswer.KeepUnlisted)
    }

    fun setVerb(
        itemIds: List<String>,
        verb: ImportItemVerb,
        candidateId: String? = null,
    ) = write { store.setImportItemsVerb(sessionId, itemIds, verb, candidateId) }

    fun retryFile(fileId: String) {
        write(retry = { retryFile(fileId) }) { store.retryImportFile(sessionId, fileId) }
    }

    fun setDateOrder(
        fileId: String,
        order: ImportDateOrder,
    ) {
        write(retry = { setDateOrder(fileId, order) }) { store.setImportFileDateOrder(sessionId, fileId, order) }
    }

    fun removeFile(fileId: String) = write { store.detachImportFile(sessionId, fileId) }

    fun retryClassification() {
        write { store.retryImportClassification(sessionId) }
    }

    fun upload(files: List<PickedFile>) {
        files.forEach { file ->
            val id = UUID.randomUUID().toString()
            pickedFiles[id] = file
            _state.update { it.copy(uploads = it.uploads + LocalUpload(id = id, name = file.name)) }
            send(id)
        }
    }

    fun retryUpload(localId: String) = send(localId)

    fun dismissUpload(localId: String) {
        pickedFiles.remove(localId)
        _state.update { it.copy(uploads = it.uploads.filterNot { upload -> upload.id == localId }) }
    }

    private fun updateUpload(
        localId: String,
        change: (LocalUpload) -> LocalUpload,
    ) = _state.update { state -> state.copy(uploads = state.uploads.map { if (it.id == localId) change(it) else it }) }

    private fun send(localId: String) {
        val file = pickedFiles[localId] ?: return
        updateUpload(localId) { it.copy(failed = false, error = null) }
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val fileId =
                    _state.value.uploads
                        .firstOrNull { it.id == localId }
                        ?.fileId
                        ?: store.uploadImportFile(file.bytes, file.mimeType, file.name).also { id ->
                            updateUpload(localId) { it.copy(fileId = id) }
                        }
                store.attachImportFiles(sessionId, listOf(fileId))
                pickedFiles.remove(localId)
                updateUpload(localId) { it.copy(attachedFileId = fileId) }
            } catch (_: ApiException.AiFeaturesDisabled) {
                updateUpload(localId) { it.copy(failed = true, error = "AI features are off") }
                _events.send(ImportSessionEvent.AiBlocked { send(localId) })
                return@launch
            } catch (e: ApiException) {
                updateUpload(localId) { it.copy(failed = true, error = e.friendlyMessage()) }
                return@launch
            }
            pollJob?.cancel()
            refresh()
        }
    }

    fun apply() {
        val items = _state.value.items ?: return
        val fingerprint = items.fingerprint ?: return
        _state.update { it.copy(applying = true) }
        write {
            try {
                store.applyImportSession(sessionId, fingerprint)
                _state.update { it.copy(applied = appliedCounts(items)) }
                store.refreshTransactions()
            } finally {
                _state.update { it.copy(applying = false) }
            }
        }
    }

    fun delete() {
        viewModelScope.launch(Dispatchers.IO) {
            pollJob?.cancel()
            generation.incrementAndGet()
            _state.update { it.copy(writes = it.writes + 1) }
            try {
                store.deleteImportSession(sessionId)
                _events.send(ImportSessionEvent.Deleted)
            } catch (e: ApiException) {
                _events.send(ImportSessionEvent.Message(e.friendlyMessage()))
                refresh()
            } finally {
                _state.update { it.copy(writes = it.writes - 1) }
            }
        }
    }
}
