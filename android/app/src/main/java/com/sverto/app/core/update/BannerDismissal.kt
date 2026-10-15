package com.sverto.app.core.update

import java.time.Instant

data class BannerDismissal(
    val versionCode: Int,
    val dismissedAt: Instant,
)
