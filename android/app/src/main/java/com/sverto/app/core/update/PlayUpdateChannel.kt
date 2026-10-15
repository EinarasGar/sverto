package com.sverto.app.core.update

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import androidx.core.net.toUri
import com.google.android.gms.tasks.Task
import com.google.android.play.core.appupdate.AppUpdateManager
import com.google.android.play.core.appupdate.AppUpdateManagerFactory
import com.google.android.play.core.appupdate.AppUpdateOptions
import com.google.android.play.core.appupdate.testing.FakeAppUpdateManager
import com.google.android.play.core.install.model.AppUpdateType
import com.google.android.play.core.install.model.InstallStatus
import com.google.android.play.core.install.model.UpdateAvailability
import com.sverto.app.BuildConfig
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

class PlayUpdateChannel private constructor(
    private val context: Context,
    private val manager: AppUpdateManager,
    private val fake: FakeAppUpdateManager?,
) : UpdateChannel {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val mutableInstallState = MutableStateFlow<InstallState>(InstallState.Idle)
    override val installState: StateFlow<InstallState> = mutableInstallState.asStateFlow()

    init {
        manager.registerListener { state ->
            mutableInstallState.value =
                installStateOf(state.installStatus(), state.bytesDownloaded(), state.totalBytesToDownload())
        }
    }

    override suspend fun check(): AvailableUpdate? {
        val info = manager.appUpdateInfo.await()
        val snapshot = installStateOf(info.installStatus(), info.bytesDownloaded(), info.totalBytesToDownload())
        if (!(mutableInstallState.value == InstallState.Downloaded && snapshot is InstallState.Downloading)) {
            mutableInstallState.value = snapshot
        }
        val availability = info.updateAvailability()
        if (availability != UpdateAvailability.UPDATE_AVAILABLE &&
            availability != UpdateAvailability.DEVELOPER_TRIGGERED_UPDATE_IN_PROGRESS
        ) {
            return null
        }
        return AvailableUpdate(
            versionCode = info.availableVersionCode(),
            urgency = UpdateUrgency.fromPriority(info.updatePriority()),
            stalenessDays = info.clientVersionStalenessDays(),
            flexibleAllowed = info.isUpdateTypeAllowed(AppUpdateType.FLEXIBLE),
            inProgress = availability == UpdateAvailability.DEVELOPER_TRIGGERED_UPDATE_IN_PROGRESS,
        )
    }

    override suspend fun startFlexible(launcher: ActivityResultLauncher<IntentSenderRequest>): UpdateFlowStart {
        val result = start(launcher, AppUpdateType.FLEXIBLE)
        if (result == UpdateFlowStart.STARTED) fake?.let(::simulateFlexibleDownload)
        return result
    }

    override suspend fun startImmediate(launcher: ActivityResultLauncher<IntentSenderRequest>): UpdateFlowStart {
        val result = start(launcher, AppUpdateType.IMMEDIATE)
        if (result == UpdateFlowStart.STARTED) fake?.userRejectsUpdate()
        return result
    }

    override fun completeInstall() {
        Log.i(TAG, "Completing in-app update")
        manager.completeUpdate().addOnFailureListener { Log.w(TAG, "Completing in-app update failed", it) }
        fake?.let {
            it.installCompletes()
            it.setUpdateNotAvailable()
        }
    }

    override fun openStoreListing() {
        val intent =
            Intent(Intent.ACTION_VIEW, "market://details?id=${context.packageName}".toUri())
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(intent) }
    }

    private suspend fun start(
        launcher: ActivityResultLauncher<IntentSenderRequest>,
        type: Int,
    ): UpdateFlowStart {
        val info =
            runCatching { manager.appUpdateInfo.await() }
                .onFailure { if (it is CancellationException) throw it }
                .getOrElse { return UpdateFlowStart.FAILED }
        val options = AppUpdateOptions.defaultOptions(type)
        if (!info.isUpdateTypeAllowed(options)) return UpdateFlowStart.NOT_ALLOWED
        Log.i(TAG, "Starting in-app update flow type=$type")
        val started =
            runCatching { manager.startUpdateFlowForResult(info, launcher, options) }
                .onFailure { if (it is CancellationException) throw it }
                .getOrDefault(false)
        return if (started) UpdateFlowStart.STARTED else UpdateFlowStart.FAILED
    }

    private fun simulateFlexibleDownload(fake: FakeAppUpdateManager) {
        scope.launch {
            fake.userAcceptsUpdate()
            fake.downloadStarts()
            fake.setTotalBytesToDownload(FAKE_DOWNLOAD_BYTES)
            for (step in 1..FAKE_DOWNLOAD_STEPS) {
                delay(FAKE_DOWNLOAD_STEP_MILLIS)
                fake.setBytesDownloaded(FAKE_DOWNLOAD_BYTES * step / FAKE_DOWNLOAD_STEPS)
            }
            fake.downloadCompletes()
        }
    }

    companion object {
        private const val TAG = "AppUpdate"
        private const val FAKE_DOWNLOAD_BYTES = 10_000_000L
        private const val FAKE_DOWNLOAD_STEPS = 10
        private const val FAKE_DOWNLOAD_STEP_MILLIS = 500L

        fun play(context: Context): PlayUpdateChannel = PlayUpdateChannel(context, AppUpdateManagerFactory.create(context), null)

        fun fake(
            context: Context,
            priority: Int,
            stalenessDays: Int?,
        ): PlayUpdateChannel {
            val fake =
                FakeAppUpdateManager(context).apply {
                    setUpdateAvailable(BuildConfig.VERSION_CODE + 1)
                    setUpdatePriority(priority)
                    setClientVersionStalenessDays(stalenessDays)
                }
            return PlayUpdateChannel(context, fake, fake)
        }
    }
}

private fun installStateOf(
    status: Int,
    bytesDownloaded: Long,
    totalBytes: Long,
): InstallState =
    when (status) {
        InstallStatus.PENDING, InstallStatus.DOWNLOADING ->
            InstallState.Downloading(if (totalBytes > 0) bytesDownloaded.toFloat() / totalBytes else null)
        InstallStatus.DOWNLOADED, InstallStatus.INSTALLING -> InstallState.Downloaded
        else -> InstallState.Idle
    }

private suspend fun <T> Task<T>.await(): T =
    suspendCancellableCoroutine { continuation ->
        addOnCompleteListener { task ->
            if (task.isSuccessful) {
                continuation.resume(task.result)
            } else {
                continuation.resumeWithException(task.exception ?: CancellationException("Play Core task was cancelled"))
            }
        }
    }
