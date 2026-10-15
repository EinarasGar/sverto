package com.sverto.app.core.update

import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

object NoUpdateChannel : UpdateChannel {
    override val installState: StateFlow<InstallState> = MutableStateFlow(InstallState.Idle).asStateFlow()

    override suspend fun check(): AvailableUpdate? = null

    override suspend fun startFlexible(launcher: ActivityResultLauncher<IntentSenderRequest>) = UpdateFlowStart.NOT_ALLOWED

    override suspend fun startImmediate(launcher: ActivityResultLauncher<IntentSenderRequest>) = UpdateFlowStart.NOT_ALLOWED

    override fun completeInstall() = Unit

    override fun openStoreListing() = Unit
}
