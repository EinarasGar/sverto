package com.sverto.app.core.ai

import android.content.Context
import android.os.Build
import android.util.Log
import com.google.mlkit.common.MlKit
import com.google.mlkit.genai.common.DownloadStatus
import com.google.mlkit.genai.common.FeatureStatus
import com.google.mlkit.genai.prompt.Generation
import com.google.mlkit.genai.prompt.GenerativeModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.sverto_core.CategoryItem
import kotlin.coroutines.cancellation.CancellationException

private const val TAG = "CategorySuggester"
private const val INFERENCE_TIMEOUT_MS = 10_000L
private const val MIN_SDK_FOR_ML_KIT = 26

class CategorySuggester(
    context: Context,
) {
    private val appContext = context.applicationContext
    private val preferences = OnDeviceAiPreferences(appContext)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val _settings = MutableStateFlow(OnDeviceAiSettings())
    val settings: StateFlow<OnDeviceAiSettings> = _settings.asStateFlow()

    private var sessionScope: CoroutineScope? = null
    private var model: GenerativeModel? = null
    private var sdkInitialized = false
    private var availability: Boolean? = null
    private var downloadStarted = false

    init {
        scope.launch {
            try {
                val enabled = preferences.isEnabled()
                if (enabled) sessionScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
                _settings.value =
                    OnDeviceAiSettings(
                        consent = if (enabled) OnDeviceAiConsent.ENABLED else OnDeviceAiConsent.DISABLED,
                    )
            } catch (e: CancellationException) {
                throw e
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.w(TAG, "Could not read on-device AI preference", e)
                _settings.value =
                    OnDeviceAiSettings(
                        consent = OnDeviceAiConsent.DISABLED,
                        errorMessage = "Could not read this device's preference. On-device AI is off.",
                    )
            }
        }
    }

    fun setEnabled(enabled: Boolean) {
        scope.launch {
            if (_settings.value.saving || _settings.value.consent == OnDeviceAiConsent.LOADING) return@launch
            _settings.value =
                _settings.value.copy(
                    consent = if (enabled) _settings.value.consent else OnDeviceAiConsent.DISABLED,
                    saving = true,
                    errorMessage = null,
                )
            if (!enabled) closeSession()
            try {
                preferences.setEnabled(enabled)
                if (enabled) sessionScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
                _settings.value =
                    OnDeviceAiSettings(
                        consent = if (enabled) OnDeviceAiConsent.ENABLED else OnDeviceAiConsent.DISABLED,
                    )
            } catch (e: CancellationException) {
                throw e
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                closeSession()
                Log.w(TAG, "Could not save on-device AI preference", e)
                _settings.value =
                    OnDeviceAiSettings(
                        consent = OnDeviceAiConsent.DISABLED,
                        errorMessage = "Could not save your choice. On-device AI is off for now. Please try again.",
                    )
            }
        }
    }

    private fun closeSession() {
        sessionScope?.cancel()
        sessionScope = null
        val previousModel = model
        model = null
        availability = null
        downloadStarted = false
        try {
            previousModel?.close()
        } catch (
            @Suppress("TooGenericExceptionCaught") e: Exception,
        ) {
            Log.w(TAG, "Could not release category suggestion model", e)
        }
    }

    private suspend fun <T> withModel(block: suspend (GenerativeModel) -> T): T? =
        withContext(Dispatchers.Main.immediate) {
            if (Build.VERSION.SDK_INT < MIN_SDK_FOR_ML_KIT) return@withContext null
            if (_settings.value.consent != OnDeviceAiConsent.ENABLED) return@withContext null
            val activeScope = sessionScope ?: return@withContext null
            try {
                val activeModel =
                    model ?: run {
                        if (!sdkInitialized) {
                            MlKit.initialize(appContext)
                            sdkInitialized = true
                        }
                        Generation.getClient().also { model = it }
                    }
                val operation = activeScope.async(start = CoroutineStart.UNDISPATCHED) { block(activeModel) }
                try {
                    operation.await().takeIf { sessionScope === activeScope }
                } finally {
                    operation.cancel()
                }
            } catch (e: CancellationException) {
                throw e
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                Log.w(TAG, "Category suggestion unavailable", e)
                null
            }
        }

    suspend fun isAvailable(): Boolean =
        withModel { activeModel ->
            availability ?: run {
                val status = activeModel.checkStatus()
                currentCoroutineContext().ensureActive()
                when (status) {
                    FeatureStatus.AVAILABLE -> true.also { availability = it }
                    FeatureStatus.UNAVAILABLE -> false.also { availability = it }
                    else -> false
                }
            }
        } ?: false

    fun prefetch(callerScope: CoroutineScope) {
        callerScope.launch {
            withModel { activeModel ->
                when (activeModel.checkStatus()) {
                    FeatureStatus.AVAILABLE -> {
                        currentCoroutineContext().ensureActive()
                        activeModel.warmup()
                    }
                    FeatureStatus.DOWNLOADABLE -> {
                        currentCoroutineContext().ensureActive()
                        if (!downloadStarted) {
                            downloadStarted = true
                            try {
                                activeModel.download().collect { status ->
                                    if (status is DownloadStatus.DownloadFailed) throw status.e
                                }
                            } catch (e: CancellationException) {
                                if (model === activeModel) downloadStarted = false
                                throw e
                            } catch (
                                @Suppress("TooGenericExceptionCaught") e: Exception,
                            ) {
                                if (model === activeModel) downloadStarted = false
                                throw e
                            }
                        }
                    }
                    else -> {}
                }
            }
        }
    }

    suspend fun suggest(
        description: String,
        categories: List<CategoryItem>,
    ): CategoryItem? =
        withModel { activeModel ->
            withTimeoutOrNull(INFERENCE_TIMEOUT_MS) {
                currentCoroutineContext().ensureActive()
                val response = activeModel.generateContent(buildCategoryPrompt(description, categories))
                currentCoroutineContext().ensureActive()
                parseCategoryResponse(
                    response.candidates
                        .firstOrNull()
                        ?.text
                        .orEmpty(),
                    categories,
                )
            }
        }
}
