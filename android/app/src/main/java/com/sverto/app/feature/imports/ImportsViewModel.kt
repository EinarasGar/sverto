package com.sverto.app.feature.imports

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import kotlinx.coroutines.Dispatchers
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
import uniffi.sverto_core.ImportSessionSummary

data class ImportsUiState(
    val sessions: List<ImportSessionSummary>? = null,
    val refreshing: Boolean = false,
    val creating: Boolean = false,
    val loadError: String? = null,
)

sealed interface ImportsEvent {
    data class Created(
        val sessionId: String,
    ) : ImportsEvent

    data object AiBlocked : ImportsEvent

    data class Message(
        val text: String,
    ) : ImportsEvent
}

class ImportsViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(ImportsUiState())
    val state: StateFlow<ImportsUiState> = _state.asStateFlow()

    private val _events = Channel<ImportsEvent>(Channel.BUFFERED)
    val events: Flow<ImportsEvent> = _events.receiveAsFlow()

    fun load(pullToRefresh: Boolean = false) {
        viewModelScope.launch(Dispatchers.IO) {
            if (pullToRefresh) _state.update { it.copy(refreshing = true) }
            try {
                val sessions = store.listImportSessions().sortedByDescending { it.createdAt }
                _state.update { it.copy(sessions = sessions, loadError = null) }
            } catch (e: ApiException) {
                if (_state.value.sessions == null) {
                    _state.update { it.copy(loadError = e.friendlyMessage()) }
                } else {
                    _events.send(ImportsEvent.Message(e.friendlyMessage()))
                }
            } finally {
                _state.update { it.copy(refreshing = false) }
            }
        }
    }

    fun create() {
        if (_state.value.creating) return
        viewModelScope.launch(Dispatchers.IO) {
            _state.update { it.copy(creating = true) }
            try {
                _events.send(ImportsEvent.Created(store.createImportSession()))
            } catch (_: ApiException.AiFeaturesDisabled) {
                _events.send(ImportsEvent.AiBlocked)
            } catch (e: ApiException) {
                _events.send(ImportsEvent.Message(e.friendlyMessage()))
            } finally {
                _state.update { it.copy(creating = false) }
            }
        }
    }
}
