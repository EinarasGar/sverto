package com.sverto.app.feature.imports

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.ImportAnswer
import uniffi.sverto_core.ImportItemVerb
import uniffi.sverto_core.ImportItems
import java.util.concurrent.atomic.AtomicLong

data class ImportItemsUiState(
    val items: ImportItems? = null,
    val loadError: String? = null,
)

class ImportItemsViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(ImportItemsUiState())
    val state: StateFlow<ImportItemsUiState> = _state.asStateFlow()

    private val _messages = Channel<String>(Channel.BUFFERED)
    val messages: Flow<String> = _messages.receiveAsFlow()

    private var sessionId = ""
    private var loadJob: Job? = null
    private val generation = AtomicLong()

    fun load(id: String) {
        sessionId = id
        loadJob?.cancel()
        loadJob =
            viewModelScope.launch(Dispatchers.IO) {
                try {
                    fetch(generation.get())
                } catch (e: ApiException) {
                    if (_state.value.items == null) {
                        _state.update { it.copy(loadError = e.friendlyMessage()) }
                    } else {
                        _messages.send(e.friendlyMessage())
                    }
                }
            }
    }

    private suspend fun fetch(startedAt: Long) {
        val items = store.listImportItems(sessionId)
        if (generation.get() == startedAt) _state.update { it.copy(items = items, loadError = null) }
    }

    private fun write(block: suspend () -> Unit) {
        viewModelScope.launch(Dispatchers.IO) {
            loadJob?.cancel()
            generation.incrementAndGet()
            try {
                block()
            } catch (e: ApiException) {
                _messages.send(e.friendlyMessage())
            }
            try {
                fetch(generation.incrementAndGet())
            } catch (e: ApiException) {
                _messages.send(e.friendlyMessage())
            }
        }
    }

    fun setVerb(
        itemId: String,
        verb: ImportItemVerb,
    ) = write { store.setImportItemsVerb(sessionId, listOf(itemId), verb, null) }

    fun keep(askKey: String) = write { store.answerImportAsk(sessionId, askKey, ImportAnswer.KeepUnlisted) }
}
