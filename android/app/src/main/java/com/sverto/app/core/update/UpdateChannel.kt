package com.sverto.app.core.update

import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import kotlinx.coroutines.flow.StateFlow

interface UpdateChannel {
    val installState: StateFlow<InstallState>

    suspend fun check(): AvailableUpdate?

    suspend fun startFlexible(launcher: ActivityResultLauncher<IntentSenderRequest>): UpdateFlowStart

    suspend fun startImmediate(launcher: ActivityResultLauncher<IntentSenderRequest>): UpdateFlowStart

    fun completeInstall()

    fun openStoreListing()
}
