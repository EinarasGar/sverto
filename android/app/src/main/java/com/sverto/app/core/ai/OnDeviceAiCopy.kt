package com.sverto.app.core.ai

object OnDeviceAiCopy {
    const val TITLE = "On-device AI"
    const val SETTING_DESCRIPTION =
        "Suggests categories using Google's ML Kit on this device. " +
            "Your transaction details stay on this device; Google receives usage data."

    const val ENABLE = "Turn on"
    const val DECLINE = "Not now"

    const val INPUTS =
        "Google ML Kit uses your transaction descriptions and category names on this device, currently to suggest categories. " +
            "ML Kit does not send those inputs or its suggestions to Google."
    const val TELEMETRY =
        "Google receives device and app information, identifiers, and ML Kit usage and performance data. " +
            "Google uses this data to operate, maintain and improve its APIs and detect abuse. " +
            "Enabling may download an AI model."
    const val OPTIONAL =
        "This is optional. You can turn it off any time in Settings. " +
            "Turning it off stops new suggestions; it does not delete data Google already received " +
            "or stop delivery of metrics already queued."

    const val PRIVACY = "Google ML Kit privacy"
    const val PRIVACY_URL = "https://developers.google.com/ml-kit/terms"
}
