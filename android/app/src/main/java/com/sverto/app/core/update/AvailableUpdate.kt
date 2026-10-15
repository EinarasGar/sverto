package com.sverto.app.core.update

data class AvailableUpdate(
    val versionCode: Int,
    val urgency: UpdateUrgency,
    val stalenessDays: Int?,
    val flexibleAllowed: Boolean,
    val inProgress: Boolean,
)
