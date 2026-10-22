package com.sverto.app.core.aifeatures

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sverto_core.ApiException

private class Harness(
    var enabled: Boolean = false,
    var enableFailure: ApiException? = null,
) {
    var enableCalls = 0
    var actionRuns = 0
    var stateDuringEnable: AiFeaturesConsentState? = null

    val gate: AiFeaturesGate =
        AiFeaturesGate(
            isEnabled = { enabled },
            enable = {
                enableCalls++
                stateDuringEnable = gate.consent.value
                enableFailure?.let { throw it }
                enabled = true
            },
        )

    val action: () -> Unit = { actionRuns++ }
}

class AiFeaturesGateTest {
    @Test
    fun enabledRunsActionWithoutPrompting() {
        val h = Harness(enabled = true)
        h.gate.runWhenEnabled(h.action)
        assertEquals(1, h.actionRuns)
        assertNull(h.gate.consent.value)
    }

    @Test
    fun disabledPromptsInsteadOfRunning() {
        val h = Harness()
        h.gate.runWhenEnabled(h.action)
        assertEquals(0, h.actionRuns)
        assertEquals(AiFeaturesConsentState(), h.gate.consent.value)
    }

    @Test
    fun turnOnEnablesThenContinuesAction() =
        runTest {
            val h = Harness()
            h.gate.runWhenEnabled(h.action)
            h.gate.turnOn()
            assertEquals(1, h.enableCalls)
            assertEquals(AiFeaturesConsentState(isEnabling = true), h.stateDuringEnable)
            assertEquals(1, h.actionRuns)
            assertNull(h.gate.consent.value)
        }

    @Test
    fun failedTurnOnKeepsPromptWithErrorAndSkipsAction() =
        runTest {
            val h = Harness(enableFailure = ApiException.Network("offline"))
            h.gate.runWhenEnabled(h.action)
            h.gate.turnOn()
            assertEquals(0, h.actionRuns)
            assertEquals(AiFeaturesConsentState(errorMessage = "offline"), h.gate.consent.value)
        }

    @Test
    fun retryAfterFailedTurnOnContinuesAction() =
        runTest {
            val h = Harness(enableFailure = ApiException.Network("offline"))
            h.gate.runWhenEnabled(h.action)
            h.gate.turnOn()
            h.enableFailure = null
            h.gate.turnOn()
            assertEquals(1, h.actionRuns)
            assertNull(h.gate.consent.value)
        }

    @Test
    fun notNowDismissesWithoutRunningOrEnabling() =
        runTest {
            val h = Harness()
            h.gate.runWhenEnabled(h.action)
            h.gate.notNow()
            h.gate.turnOn()
            assertNull(h.gate.consent.value)
            assertEquals(0, h.enableCalls)
            assertEquals(0, h.actionRuns)
        }

    @Test
    fun promptToEnableAsksEvenWhenFlagSaysEnabled() =
        runTest {
            val h = Harness(enabled = true)
            h.gate.promptToEnable(h.action)
            assertEquals(0, h.actionRuns)
            assertEquals(AiFeaturesConsentState(), h.gate.consent.value)
            h.gate.turnOn()
            assertEquals(1, h.enableCalls)
            assertEquals(1, h.actionRuns)
        }

    @Test
    fun newPromptReplacesPendingAction() =
        runTest {
            val h = Harness()
            var firstRuns = 0
            h.gate.runWhenEnabled { firstRuns++ }
            h.gate.runWhenEnabled(h.action)
            h.gate.turnOn()
            assertEquals(0, firstRuns)
            assertEquals(1, h.actionRuns)
            assertTrue(h.enabled)
        }
}
