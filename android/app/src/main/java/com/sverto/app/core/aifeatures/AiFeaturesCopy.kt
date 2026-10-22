package com.sverto.app.core.aifeatures

object AiFeaturesCopy {
    const val TITLE = "AI features"
    const val SETTING_DESCRIPTION =
        "Myra chat and receipt scanning use Google's Gemini AI. " +
            "When off, none of your data is sent to Gemini and these features are unavailable."

    const val ENABLE = "Turn on"
    const val ONBOARDING_DECLINE = "Keep off"

    const val POPUP_TITLE = "Turn on AI features?"
    const val POPUP_DECLINE = "Not now"

    const val INTRO = "Myra chat and receipt scanning use Google's Gemini AI. To use them, Sverto sends Google:"
    val SHARED_DATA =
        listOf(
            "Your Myra messages and any images or receipts you attach",
            "Account, transaction, holding and category details Myra looks up to answer you",
            "Your transaction descriptions, so Myra can search them",
        )
    const val FOOTER =
        "Google does not use this data to improve its products. You can use the rest of Sverto without AI features."

    const val PRIVACY_POLICY = "Privacy policy"
    const val PRIVACY_POLICY_URL = "https://sverto.com/privacy"
}
