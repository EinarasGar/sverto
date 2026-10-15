package com.sverto.app.core.update

import java.time.Duration
import java.time.Instant

object UpdatePolicy {
    private const val GRACE_PERIOD_DAYS = 7
    private val HOME_BANNER_SNOOZE = Duration.ofDays(7)

    fun present(
        update: AvailableUpdate?,
        installState: InstallState,
        autoPromptedVersionCode: Int?,
        bannerDismissal: BannerDismissal?,
        now: Instant,
    ): UpdatePresentation {
        if (update == null || update.urgency == UpdateUrgency.NONE) return UpdatePresentation()
        if (update.urgency == UpdateUrgency.REQUIRED) return UpdatePresentation(blocking = true)
        val stale = (update.stalenessDays ?: 0) >= GRACE_PERIOD_DAYS
        val snoozed =
            bannerDismissal?.versionCode == update.versionCode &&
                now.isBefore(bannerDismissal.dismissedAt.plus(HOME_BANNER_SNOOZE)) &&
                installState != InstallState.Downloaded
        return UpdatePresentation(
            showBadge = true,
            showSettingsBanner = true,
            showHomeBanner =
                !snoozed &&
                    (
                        update.urgency >= UpdateUrgency.NUDGE ||
                            (update.urgency == UpdateUrgency.GRACE_NUDGE && stale)
                    ),
            installState = installState,
            autoPromptFlexible =
                update.urgency == UpdateUrgency.PROMPT &&
                    installState == InstallState.Idle &&
                    update.flexibleAllowed &&
                    autoPromptedVersionCode != update.versionCode,
        )
    }
}
