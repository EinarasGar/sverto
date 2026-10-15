package com.sverto.app.core.update

import android.content.Context
import android.os.Build
import com.sverto.app.BuildConfig

object UpdateChannelSelector {
    private const val PLAY_STORE_PACKAGE = "com.android.vending"

    fun select(context: Context): UpdateChannel =
        when {
            BuildConfig.FAKE_UPDATE_PRIORITY >= 0 ->
                PlayUpdateChannel.fake(
                    context = context,
                    priority = BuildConfig.FAKE_UPDATE_PRIORITY,
                    stalenessDays = BuildConfig.FAKE_UPDATE_STALENESS_DAYS.takeIf { it >= 0 },
                )
            installerPackage(context) == PLAY_STORE_PACKAGE -> PlayUpdateChannel.play(context)
            else -> NoUpdateChannel
        }

    private fun installerPackage(context: Context): String? =
        runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                context.packageManager.getInstallSourceInfo(context.packageName).installingPackageName
            } else {
                @Suppress("DEPRECATION")
                context.packageManager.getInstallerPackageName(context.packageName)
            }
        }.getOrNull()
}
