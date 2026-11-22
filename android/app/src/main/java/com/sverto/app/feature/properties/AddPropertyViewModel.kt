package com.sverto.app.feature.properties

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import com.sverto.app.feature.accounts.SubmitState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sverto_core.AccountListItem
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.AssetItem
import uniffi.sverto_core.CreatePropertyInput
import uniffi.sverto_core.PropertyKind
import uniffi.sverto_core.PropertyPurchaseInput

sealed interface LoanCandidates {
    data object Loading : LoanCandidates

    data class Loaded(
        val accounts: List<AccountListItem>,
    ) : LoanCandidates

    data object Failed : LoanCandidates
}

class AddPropertyViewModel(
    private val store: AppStore,
) : ViewModel() {
    val kind = MutableStateFlow(PropertyKind.REAL_ESTATE)
    val name = MutableStateFlow("")
    val currencyAssetId = MutableStateFlow(store.getDefaultAssetId())
    val currencyTicker = MutableStateFlow(store.getDefaultAssetTicker().orEmpty())
    val value = MutableStateFlow("")
    val ownershipShare = MutableStateFlow(1.0f)
    val purchaseDateMillis = MutableStateFlow(todayUtcMillis())
    val boughtBeforeToday: StateFlow<Boolean> =
        purchaseDateMillis
            .map { it < todayUtcMillis() }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), false)
    val purchasePrice = MutableStateFlow("")
    val loanAccountIds = MutableStateFlow<Set<String>>(emptySet())

    private val _loanCandidates = MutableStateFlow<LoanCandidates>(LoanCandidates.Loading)
    val loanCandidates: StateFlow<LoanCandidates> = _loanCandidates.asStateFlow()

    private val _submitState = MutableStateFlow<SubmitState>(SubmitState.Idle)
    val submitState: StateFlow<SubmitState> = _submitState.asStateFlow()

    init {
        loadLoanCandidates()
    }

    fun loadLoanCandidates() {
        _loanCandidates.value = LoanCandidates.Loading
        viewModelScope.launch(Dispatchers.IO) {
            _loanCandidates.value =
                try {
                    LoanCandidates.Loaded(store.getLoanCandidates())
                } catch (_: ApiException) {
                    LoanCandidates.Failed
                }
        }
    }

    fun selectCurrency(currency: AssetItem) {
        currencyAssetId.value = currency.id
        currencyTicker.value = currency.ticker
    }

    fun toggleLoan(accountId: String) {
        val ids = loanAccountIds.value
        loanAccountIds.value = if (accountId in ids) ids - accountId else ids + accountId
    }

    fun save(onSuccess: () -> Unit) {
        val currencyId = currencyAssetId.value ?: return
        val price = purchasePrice.value.trim().toDoubleOrNull() ?: return
        val beforeToday = boughtBeforeToday.value
        val amount =
            if (beforeToday) {
                value.value.trim().toDoubleOrNull() ?: return
            } else {
                price
            }
        val input =
            CreatePropertyInput(
                kind = kind.value,
                name = name.value.trim(),
                currencyAssetId = currencyId,
                ownershipShare = ownershipShare.value.toDouble(),
                value = amount,
                purchase =
                    if (beforeToday) {
                        PropertyPurchaseInput(date = pickedDateSeconds(purchaseDateMillis.value), price = price)
                    } else {
                        null
                    },
                loanAccountIds = loanAccountIds.value.toList(),
            )

        _submitState.value = SubmitState.Loading
        viewModelScope.launch {
            withContext(Dispatchers.IO) { runCatching { store.createProperty(input) } }.fold(
                onSuccess = {
                    _submitState.value = SubmitState.Idle
                    onSuccess()
                },
                onFailure = { e -> _submitState.value = SubmitState.Error(e.friendlyMessage()) },
            )
        }
    }

    fun clearError() {
        if (_submitState.value is SubmitState.Error) {
            _submitState.value = SubmitState.Idle
        }
    }
}
