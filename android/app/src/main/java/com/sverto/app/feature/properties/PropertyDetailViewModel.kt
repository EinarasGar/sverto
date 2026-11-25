package com.sverto.app.feature.properties

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import com.sverto.app.core.launchCatching
import com.sverto.app.feature.portfolio.ChartPoint
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.PropertyDetail
import uniffi.sverto_core.UpdatePropertyInput

sealed interface PurchaseDate {
    data object Loading : PurchaseDate

    data class Loaded(
        val seconds: Long,
    ) : PurchaseDate

    data object Failed : PurchaseDate
}

data class PropertyDetailUiState(
    val isLoading: Boolean = true,
    val detail: PropertyDetail? = null,
    val loanCandidates: LoanCandidates = LoanCandidates.Loading,
    val currencyTicker: String = "",
    val trend: List<ChartPoint>? = null,
    val valuations: List<ChartPoint> = emptyList(),
    val chartFailed: Boolean = false,
    val isRefreshing: Boolean = false,
    val purchaseDate: PurchaseDate = PurchaseDate.Loading,
    val editError: String? = null,
    val isSavingEdit: Boolean = false,
)

class PropertyDetailViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(PropertyDetailUiState())
    val state = _state.asStateFlow()

    private val _errors = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val errors = _errors.asSharedFlow()

    private val _saved = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val saved = _saved.asSharedFlow()

    private var propertyId: String = ""
    private var assetId: Int = 0

    fun load(
        propertyId: String,
        assetId: Int,
    ) {
        this.propertyId = propertyId
        this.assetId = assetId
        viewModelScope.launch(Dispatchers.IO) { reload() }
    }

    fun refresh() {
        _state.update { it.copy(isRefreshing = true) }
        viewModelScope.launch(Dispatchers.IO) {
            reload()
            _state.update { it.copy(isRefreshing = false) }
        }
    }

    private suspend fun reload() {
        try {
            // The detail and chart read memory-cached responses, which would hide edits made
            // elsewhere (a loan's own page, another device) when returning here.
            store.refreshProperty(assetId)
            reloadDetail()
        } catch (e: ApiException) {
            _state.update { it.copy(isLoading = false) }
            _errors.emit(e.friendlyMessage())
        }
    }

    fun retryChart() {
        viewModelScope.launch(Dispatchers.IO) { loadChart() }
    }

    fun loadLoanCandidates() {
        _state.update { it.copy(loanCandidates = LoanCandidates.Loading) }
        viewModelScope.launch(Dispatchers.IO) {
            val candidates =
                try {
                    LoanCandidates.Loaded(store.getLoanCandidates())
                } catch (_: ApiException) {
                    LoanCandidates.Failed
                }
            _state.update { it.copy(loanCandidates = candidates) }
        }
    }

    fun addValuation(
        date: Long,
        value: Double,
    ) = launchCatching(_errors) {
        val summary = _state.value.detail?.summary ?: return@launchCatching
        store.addPropertyValuation(summary.assetId, summary.currencyAssetId, date, value)
        _saved.emit(Unit)
        reloadDetail()
    }

    fun startEdit() {
        _state.update { it.copy(editError = null) }
        loadPurchaseDate()
    }

    fun loadPurchaseDate() {
        val detail = _state.value.detail ?: return
        _state.update { it.copy(purchaseDate = PurchaseDate.Loading) }
        viewModelScope.launch(Dispatchers.IO) {
            val purchaseDate =
                try {
                    PurchaseDate.Loaded(store.getPropertyPurchaseDate(detail.account.id, detail.summary.assetId))
                } catch (_: ApiException) {
                    PurchaseDate.Failed
                }
            _state.update { it.copy(purchaseDate = purchaseDate) }
        }
    }

    fun saveEdit(
        name: String,
        ownershipShare: Double,
        purchaseDate: Long?,
        onSaved: () -> Unit,
    ) {
        val detail = _state.value.detail ?: return
        _state.update { it.copy(isSavingEdit = true, editError = null) }
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    store.updateProperty(propertyId, UpdatePropertyInput(name = name, ownershipShare = ownershipShare))
                    if (purchaseDate != null) {
                        store.setPropertyPurchaseDate(
                            detail.account.id,
                            detail.summary.assetId,
                            detail.summary.currencyAssetId,
                            purchaseDate,
                        )
                    }
                    reloadDetail()
                }
                onSaved()
            } catch (e: ApiException) {
                val message =
                    when (e) {
                        is ApiException.Network, is ApiException.Timeout -> "Couldn't save. Check your connection and try again."
                        else -> e.friendlyMessage()
                    }
                _state.update { it.copy(editError = message) }
            } finally {
                _state.update { it.copy(isSavingEdit = false) }
            }
        }
    }

    fun linkLoan(accountId: String) =
        launchCatching(_errors) {
            store.linkPropertyLoan(propertyId, accountId)
            _saved.emit(Unit)
            reloadDetail()
        }

    fun unlinkLoan(accountId: String) =
        launchCatching(_errors) {
            store.unlinkPropertyLoan(propertyId, accountId)
            reloadDetail()
        }

    private suspend fun reloadDetail() {
        val detail = store.getPropertyDetail(propertyId)
        val currencyTicker =
            _state.value.currencyTicker.ifEmpty {
                store
                    .getAllCurrencies()
                    .firstOrNull { it.id == detail.summary.currencyAssetId }
                    ?.ticker
                    .orEmpty()
            }
        _state.update { it.copy(isLoading = false, detail = detail, currencyTicker = currencyTicker) }
        loadChart()
    }

    private suspend fun loadChart() {
        val summary = _state.value.detail?.summary ?: return
        _state.update { it.copy(chartFailed = false) }
        try {
            val chart = store.getPropertyValueChart(summary.assetId, summary.currencyAssetId, summary.value)
            _state.update {
                it.copy(
                    trend = chart.line.map { point -> ChartPoint(date = point.timestamp, value = point.value) },
                    valuations = chart.valuations.map { point -> ChartPoint(date = point.timestamp, value = point.value) },
                )
            }
        } catch (_: ApiException) {
            _state.update { it.copy(chartFailed = true) }
        }
    }
}
