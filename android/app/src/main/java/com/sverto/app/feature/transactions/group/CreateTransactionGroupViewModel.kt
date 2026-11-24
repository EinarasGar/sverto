package com.sverto.app.feature.transactions.group

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.ai.CategorySuggester
import com.sverto.app.core.ai.CategorySuggestionController
import com.sverto.app.core.ai.OnDeviceAiConsent
import com.sverto.app.core.ai.SuggestionState
import com.sverto.app.core.friendlyMessage
import com.sverto.app.feature.transactions.create.CorrectionState
import com.sverto.app.feature.transactions.create.CorrectionTypeChange
import com.sverto.app.feature.transactions.create.apiTypeToConfigKey
import com.sverto.app.feature.transactions.create.getTransactionTypeConfig
import com.sverto.app.feature.transactions.quickupload.LedgerMatches
import com.sverto.app.feature.transactions.quickupload.proposalToGroupFormState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.CategoryItem
import uniffi.sverto_core.CreateTransactionGroupInput
import uniffi.sverto_core.CreateTransactionInput
import uniffi.sverto_core.QuickUploadReceipt
import uniffi.sverto_core.QuickUploadReceiptLine
import uniffi.sverto_core.TransactionListItem
import java.math.BigDecimal

private const val TAG = "CreateGroupVM"

class CreateTransactionGroupViewModel(
    private val store: AppStore,
    private val categorySuggester: CategorySuggester,
) : ViewModel() {
    private var editGroupId: String? = null
    private var fromSelection = false
    private var initialized = false

    private val _formState = MutableStateFlow(GroupFormState())
    val formState: StateFlow<GroupFormState> = _formState.asStateFlow()

    @Volatile
    private var allCategories: List<CategoryItem> = emptyList()
    private val _categoryResults = MutableStateFlow<List<CategoryItem>>(emptyList())
    val categoryResults: StateFlow<List<CategoryItem>> = _categoryResults.asStateFlow()

    private val _categoriesLoading = MutableStateFlow(false)
    val categoriesLoading: StateFlow<Boolean> = _categoriesLoading.asStateFlow()

    private val _submitState = MutableStateFlow(GroupSubmitState.IDLE)
    val submitState: StateFlow<GroupSubmitState> = _submitState.asStateFlow()

    private val _isLoading = MutableStateFlow(false)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _errorMessage = MutableStateFlow<String?>(null)
    val errorMessage: StateFlow<String?> = _errorMessage.asStateFlow()

    private val _quickUploadId = MutableStateFlow<String?>(null)
    val quickUploadId: StateFlow<String?> = _quickUploadId.asStateFlow()

    private val _ledgerMatches = MutableStateFlow(LedgerMatches())
    val ledgerMatches: StateFlow<LedgerMatches> = _ledgerMatches.asStateFlow()

    private val _correctionState = MutableStateFlow(CorrectionState.IDLE)
    val correctionState: StateFlow<CorrectionState> = _correctionState.asStateFlow()

    private val _correctionTypeChange = MutableStateFlow<CorrectionTypeChange?>(null)
    val correctionTypeChange: StateFlow<CorrectionTypeChange?> = _correctionTypeChange.asStateFlow()

    private val _correctionBlockedByAiFeatures = MutableStateFlow<String?>(null)
    val correctionBlockedByAiFeatures: StateFlow<String?> = _correctionBlockedByAiFeatures.asStateFlow()

    private var categorySearchJob: Job? = null

    private val suggestionController =
        CategorySuggestionController(
            isAvailable = { categorySuggester.isAvailable() },
            loadCategories = { ensureCategories() },
            suggest = { description, categories -> categorySuggester.suggest(description, categories) },
        )
    val suggestionState: StateFlow<SuggestionState> = suggestionController.state

    init {
        viewModelScope.launch {
            categorySuggester.settings
                .map { it.consent }
                .distinctUntilChanged()
                .collect { consent ->
                    if (consent == OnDeviceAiConsent.ENABLED) {
                        categorySuggester.prefetch(viewModelScope)
                    } else {
                        suggestionController.reset()
                    }
                }
        }
    }

    private suspend fun ensureCategories(): List<CategoryItem> {
        if (allCategories.isEmpty()) {
            allCategories = store.getAllCategories()
        }
        return allCategories
    }

    fun onDescriptionCommitted() {
        viewModelScope.launch {
            val consent = categorySuggester.settings.value
            if (consent.consent != OnDeviceAiConsent.ENABLED) return@launch
            val committed = _formState.value.description
            val suggested = withContext(Dispatchers.IO) { suggestionController.suggestFor(committed) } ?: return@launch
            if (categorySuggester.settings.value !== consent) return@launch
            val fresh = _formState.value
            if (fresh.description != committed) return@launch
            if (fresh.categoryId == null || fresh.categoryFromAi) {
                _formState.update { state ->
                    if (state.description == committed && (state.categoryId == null || state.categoryFromAi)) {
                        state.copy(
                            categoryId = suggested.id,
                            categoryName = suggested.name,
                            categoryFromAi = true,
                        )
                    } else {
                        state
                    }
                }
            } else if (fresh.categoryId != suggested.id) {
                suggestionController.offer(suggested)
            }
        }
    }

    fun applySuggestion() {
        if (categorySuggester.settings.value.consent != OnDeviceAiConsent.ENABLED) return
        val category = suggestionController.takeSuggestion() ?: return
        _formState.update { state ->
            state.copy(
                categoryId = category.id,
                categoryName = category.name,
                categoryFromAi = true,
            )
        }
    }

    fun init() {
        if (initialized) return
        initialized = true
        editGroupId = null
        _submitState.value = GroupSubmitState.IDLE
        _errorMessage.value = null
        _categoryResults.value = emptyList()
        _isLoading.value = false
        _formState.value = GroupFormState(date = System.currentTimeMillis() / 1000)
    }

    fun initForEdit(group: TransactionListItem) {
        if (initialized) return
        initialized = true
        editGroupId = group.id
        _submitState.value = GroupSubmitState.IDLE
        _errorMessage.value = null
        _categoryResults.value = emptyList()
        _isLoading.value = true

        _formState.value =
            GroupFormState(
                date = group.date,
                description = group.description,
                categoryId = group.categoryId,
                categoryName = group.categoryName,
            )
        suggestionController.markCommitted(group.description)

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val items =
                    group.children
                        .map { child -> async { loadGroupItem(child.id) } }
                        .awaitAll()
                _formState.value = _formState.value.copy(transactions = items)
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.e(TAG, "Edit init failed", e)
                _errorMessage.value = e.message ?: "Unknown error"
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun initFromSelection(transactionIds: List<String>) {
        if (initialized) return
        initialized = true
        editGroupId = null
        fromSelection = true
        _submitState.value = GroupSubmitState.IDLE
        _errorMessage.value = null
        _categoryResults.value = emptyList()
        _isLoading.value = true
        _formState.value = GroupFormState(date = System.currentTimeMillis() / 1000)

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val items =
                    transactionIds
                        .map { id -> async { loadGroupItem(id) } }
                        .awaitAll()
                _formState.value = _formState.value.copy(transactions = items)
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.e(TAG, "Selection init failed", e)
                _errorMessage.value = e.message ?: "Unknown error"
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun initFromProposal(quickUploadId: String) {
        if (initialized) return
        initialized = true
        editGroupId = null
        _submitState.value = GroupSubmitState.IDLE
        _errorMessage.value = null
        _categoryResults.value = emptyList()
        _isLoading.value = true

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val detail = store.getQuickUploadDetail(quickUploadId)
                val formState =
                    proposalToGroupFormState(
                        detail.proposalData ?: "{}",
                        detail.lookupTables,
                    )
                loadLedgerMatches(quickUploadId)
                _formState.value = formState
                suggestionController.markCommitted(formState.description)
                _quickUploadId.value = quickUploadId
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.e(TAG, "Proposal init failed", e)
                _errorMessage.value = e.message ?: "Failed to load proposal"
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun sendCorrection(message: String) {
        val uploadId = _quickUploadId.value ?: return
        _correctionState.value = CorrectionState.SENDING
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val detail = store.sendQuickUploadCorrection(uploadId, message)
                if (detail.proposalType != null && detail.proposalType != "transaction_group") {
                    _correctionState.value = CorrectionState.IDLE
                    _correctionTypeChange.value = CorrectionTypeChange(uploadId, detail.proposalType!!)
                    return@launch
                }
                val corrected =
                    proposalToGroupFormState(
                        detail.proposalData ?: "{}",
                        detail.lookupTables,
                    )
                _formState.value = corrected
                suggestionController.markCommitted(corrected.description)
                loadLedgerMatches(uploadId)
                _correctionState.value = CorrectionState.UPDATED
                delay(2000)
                _correctionState.value = CorrectionState.IDLE
            } catch (ignored: ApiException.AiFeaturesDisabled) {
                _correctionState.value = CorrectionState.IDLE
                _correctionBlockedByAiFeatures.value = message
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.e(TAG, "Correction failed", e)
                _correctionState.value = CorrectionState.IDLE
            }
        }
    }

    fun clearCorrectionBlockedByAiFeatures() {
        _correctionBlockedByAiFeatures.value = null
    }

    private suspend fun loadLedgerMatches(uploadId: String) {
        val candidates =
            try {
                store.getQuickUploadMatches(uploadId)
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.e(TAG, "Loading ledger matches failed", e)
                emptyList()
            }
        _ledgerMatches.update { it.replacedBy(candidates) }
    }

    fun showNextLedgerMatch() {
        _ledgerMatches.update { it.selectingNext() }
    }

    fun setMergeIntoLedgerMatch(merge: Boolean) {
        _ledgerMatches.update { it.copy(merge = merge) }
    }

    fun updateDate(millis: Long) {
        _formState.value = _formState.value.copy(date = millis / 1000)
    }

    fun updateDescription(value: String) {
        suggestionController.onDescriptionChanged(value)
        _formState.value = _formState.value.copy(description = value)
    }

    fun selectCategory(item: CategoryItem) {
        suggestionController.reset()
        _formState.value =
            _formState.value.copy(
                categoryId = item.id,
                categoryName = item.name,
                categoryFromAi = false,
            )
        _categoryResults.value = emptyList()
    }

    fun loadCategories() {
        categorySearchJob?.cancel()
        categorySearchJob =
            viewModelScope.launch(Dispatchers.IO) {
                _categoriesLoading.value = true
                try {
                    allCategories = store.getAllCategories()
                    _categoryResults.value = allCategories
                } catch (
                    @Suppress("TooGenericExceptionCaught") e: Exception,
                ) {
                    Log.e(TAG, "Loading categories failed", e)
                } finally {
                    _categoriesLoading.value = false
                }
            }
    }

    fun filterCategories(query: String) {
        _categoryResults.value =
            if (query.isBlank()) {
                allCategories
            } else {
                allCategories.filter { it.name.contains(query, ignoreCase = true) }
            }
    }

    fun addTransaction(item: GroupTransactionItem) {
        _formState.value =
            _formState.value.copy(
                transactions = _formState.value.transactions + item,
            )
    }

    fun updateTransaction(
        index: Int,
        item: GroupTransactionItem,
    ) {
        val updated = _formState.value.transactions.toMutableList()
        if (index in updated.indices) {
            updated[index] = item
            _formState.value = _formState.value.copy(transactions = updated)
        }
    }

    fun removeTransaction(index: Int) {
        val updated = _formState.value.transactions.toMutableList()
        if (index in updated.indices) {
            updated.removeAt(index)
            _formState.value = _formState.value.copy(transactions = updated)
        }
    }

    fun submit() {
        val state = _formState.value
        val validationError =
            when {
                state.date == null -> "Date is required"
                state.categoryId == null -> "Category is required"
                state.transactions.isEmpty() -> "Add at least one transaction"
                state.description.isBlank() -> "Description is required"
                else -> null
            }
        if (validationError != null) {
            _errorMessage.value = validationError
            return
        }

        val qId = _quickUploadId.value
        val mergeTargetId = _ledgerMatches.value.mergeTargetId
        if (qId != null && mergeTargetId != null) {
            mergeIntoLedgerMatch(qId, mergeTargetId, state)
            return
        }

        _submitState.value = GroupSubmitState.SUBMITTING
        _errorMessage.value = null

        val input =
            CreateTransactionGroupInput(
                date = state.date!!,
                description = state.description,
                categoryId = state.categoryId!!,
                transactions = state.transactions.map { it.input },
            )

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val groupId = editGroupId
                when {
                    groupId != null -> store.updateTransactionGroup(groupId, input)
                    fromSelection -> store.groupIndividualTransactions(input)
                    else -> store.createTransactionGroup(input)
                }
                val qId = _quickUploadId.value
                if (qId != null) {
                    store.completeQuickUpload(qId, true)
                    _quickUploadId.value = null
                }
                _submitState.value = GroupSubmitState.SUCCESS
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.e(TAG, "Submit failed", e)
                _errorMessage.value = e.message ?: "Unknown error"
                _submitState.value = GroupSubmitState.IDLE
            }
        }
    }

    private fun mergeIntoLedgerMatch(
        uploadId: String,
        transactionId: String,
        state: GroupFormState,
    ) {
        if (state.transactions.any { it.input.typeKey != "regular" }) {
            _errorMessage.value = "Only spending lines can be merged into a transaction"
            return
        }
        val categoryId = state.categoryId!!
        val receipt =
            QuickUploadReceipt(
                date = state.date!!,
                description = state.description,
                categoryId = categoryId,
                lines =
                    state.transactions.map { item ->
                        QuickUploadReceiptLine(
                            description = item.input.description,
                            categoryId = item.input.categoryId ?: categoryId,
                            amount = item.input.primaryAmount,
                        )
                    },
            )
        _submitState.value = GroupSubmitState.SUBMITTING
        _errorMessage.value = null

        viewModelScope.launch(Dispatchers.IO) {
            try {
                store.mergeQuickUpload(uploadId, transactionId, receipt)
                _submitState.value = GroupSubmitState.SUCCESS
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.e(TAG, "Merge failed", e)
                _errorMessage.value = e.friendlyMessage()
                _submitState.value = GroupSubmitState.IDLE
            }
        }
    }

    private suspend fun loadGroupItem(transactionId: String): GroupTransactionItem {
        val editable = store.getEditableTransaction(transactionId)
        val config = getTransactionTypeConfig(apiTypeToConfigKey(editable.typeKey))
        return GroupTransactionItem(
            input =
                CreateTransactionInput(
                    transactionId = transactionId,
                    typeKey = editable.typeKey,
                    date = editable.date,
                    primaryEntryId = editable.primaryEntryId,
                    primaryAccountId = editable.primaryAccountId,
                    primaryAssetId = editable.primaryAssetId,
                    primaryAmount = editable.primaryAmount,
                    secondaryEntryId = editable.secondaryEntryId,
                    secondaryAccountId = editable.secondaryAccountId,
                    secondaryAssetId = editable.secondaryAssetId,
                    secondaryAmount = editable.secondaryAmount,
                    originAssetId = editable.originAssetId,
                    categoryId = editable.categoryId,
                    description = editable.description.ifBlank { null },
                ),
            descriptionDisplay = editable.description.ifBlank { config.label },
            typeLabel = config.label,
            amountDisplay =
                BigDecimal
                    .valueOf(editable.primaryAmount)
                    .stripTrailingZeros()
                    .toPlainString(),
            accountName = editable.primaryAccountName,
            assetDisplay = editable.primaryAssetDisplay,
            categoryName = editable.categoryName,
        )
    }
}
