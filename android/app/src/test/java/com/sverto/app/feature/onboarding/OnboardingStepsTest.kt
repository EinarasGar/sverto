package com.sverto.app.feature.onboarding

import org.junit.Assert.assertEquals
import org.junit.Test

class OnboardingStepsTest {
    @Test
    fun newUser_seesAllStepsUpToCurrentVersion() {
        val steps = stepsForVersion(0)
        assertEquals(
            listOf(
                OnboardingStepId.WELCOME,
                OnboardingStepId.BASE_CURRENCY,
                OnboardingStepId.AI_FEATURES,
                OnboardingStepId.NOTIFICATIONS,
            ),
            steps,
        )
    }

    @Test
    fun versionOneUser_seesOnlyAiFeaturesStep() {
        assertEquals(listOf(OnboardingStepId.AI_FEATURES), stepsForVersion(1))
    }

    @Test
    fun fullyOnboardedUser_seesNoSteps() {
        assertEquals(emptyList<OnboardingStepId>(), stepsForVersion(CURRENT_ONBOARDING_VERSION))
    }

    @Test
    fun currentVersionIsTwo() {
        assertEquals(2, CURRENT_ONBOARDING_VERSION)
    }
}
