package com.sverto.app.core.update

import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.Instant

class AppUpdateController(
    private val channel: UpdateChannel,
    private val preferences: UpdatePreferences,
) : DefaultLifecycleObserver {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val availableUpdate = MutableStateFlow<AvailableUpdate?>(null)
    private val autoPromptedVersionCode = MutableStateFlow<Int?>(null)
    private val bannerDismissal = MutableStateFlow<BannerDismissal?>(null)
    private val now = MutableStateFlow(Instant.now())
    private var promptHistoryLoaded = false
    private var launcher: ActivityResultLauncher<IntentSenderRequest>? = null
    private var refreshJob: Job? = null
    private var flowJob: Job? = null
    private var restartSafe = false
    private var leftForAnotherActivity = false
    private var requiredFlowLaunched = false

    val presentation: StateFlow<UpdatePresentation> =
        combine(
            availableUpdate,
            channel.installState,
            autoPromptedVersionCode,
            bannerDismissal,
            now,
            UpdatePolicy::present,
        ).stateIn(scope, SharingStarted.Eagerly, UpdatePresentation())

    init {
        ProcessLifecycleOwner.get().lifecycle.addObserver(this)
    }

    fun attach(launcher: ActivityResultLauncher<IntentSenderRequest>) {
        this.launcher = launcher
    }

    fun detach(launcher: ActivityResultLauncher<IntentSenderRequest>) {
        if (this.launcher === launcher) this.launcher = null
    }

    fun setRestartSafe(safe: Boolean) {
        restartSafe = safe
    }

    fun onLeavingForActivity() {
        leftForAnotherActivity = true
    }

    fun onActivityResumed() {
        leftForAnotherActivity = false
    }

    fun onBannerClick() {
        when (channel.installState.value) {
            InstallState.Downloaded -> completeInstall()
            is InstallState.Downloading -> Unit
            InstallState.Idle -> startUserRequestedFlow(channel::startFlexible)
        }
    }

    fun startRequiredUpdate() {
        startUserRequestedFlow(channel::startImmediate)
    }

    fun dismissHomeBanner() {
        val update = availableUpdate.value ?: return
        val dismissal = BannerDismissal(update.versionCode, Instant.now())
        now.value = dismissal.dismissedAt
        bannerDismissal.value = dismissal
        scope.launch { preferences.setBannerDismissal(dismissal) }
    }

    fun autoPrompt() {
        if (!isInForeground() || flowJob?.isActive == true) return
        val launcher = launcher ?: return
        val update = availableUpdate.value ?: return
        if (autoPromptedVersionCode.value == update.versionCode) return
        autoPromptedVersionCode.value = update.versionCode
        flowJob =
            scope.launch {
                preferences.setAutoPromptedVersionCode(update.versionCode)
                channel.startFlexible(launcher)
            }
    }

    override fun onStart(owner: LifecycleOwner) {
        restartRefresh()
    }

    override fun onStop(owner: LifecycleOwner) {
        if (restartSafe && !leftForAnotherActivity && channel.installState.value == InstallState.Downloaded) {
            completeInstall()
        }
    }

    private fun restartRefresh() {
        refreshJob?.cancel()
        refreshJob = scope.launch { refresh() }
    }

    private suspend fun refresh() {
        now.value = Instant.now()
        if (!promptHistoryLoaded) {
            val loaded = preferences.autoPromptedVersionCode()
            autoPromptedVersionCode.compareAndSet(null, loaded)
            bannerDismissal.compareAndSet(null, preferences.bannerDismissal())
            promptHistoryLoaded = true
        }
        val update =
            runCatching { channel.check() }
                .onFailure { if (it is CancellationException) throw it }
                .getOrElse { return }
        availableUpdate.value = update
        val launcher = launcher ?: return
        val required = update?.urgency == UpdateUrgency.REQUIRED && (update.inProgress || !requiredFlowLaunched)
        if (required && isInForeground()) {
            requiredFlowLaunched = true
            scope.launch { channel.startImmediate(launcher) }
        }
    }

    private fun startUserRequestedFlow(start: suspend (ActivityResultLauncher<IntentSenderRequest>) -> UpdateFlowStart) {
        if (flowJob?.isActive == true) return
        val launcher = launcher ?: return
        flowJob =
            scope.launch {
                if (start(launcher) == UpdateFlowStart.NOT_ALLOWED) channel.openStoreListing()
            }
    }

    private fun completeInstall() {
        channel.completeInstall()
        restartRefresh()
    }

    private fun isInForeground(): Boolean =
        ProcessLifecycleOwner
            .get()
            .lifecycle.currentState
            .isAtLeast(Lifecycle.State.STARTED)
}
