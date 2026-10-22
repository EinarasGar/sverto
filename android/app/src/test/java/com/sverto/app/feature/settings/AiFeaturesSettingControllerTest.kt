package com.sverto.app.feature.settings

import com.sverto.app.core.aifeatures.AiFeaturesConsentState
import com.sverto.app.core.aifeatures.AiFeaturesGate
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sverto_core.ApiException

private class Harness(
    var flag: Boolean = false,
    var disableFailure: ApiException? = null,
) {
    var enableCalls = 0
    var disableCalls = 0
    var disableGate: CompletableDeferred<Unit>? = null

    val gate =
        AiFeaturesGate(
            isEnabled = { flag },
            enable = {
                enableCalls++
                flag = true
            },
        )

    val controller =
        AiFeaturesSettingController(
            gate = gate,
            isEnabled = { flag },
            disable = {
                disableCalls++
                disableGate?.await()
                disableFailure?.let { throw it }
                flag = false
            },
        )
}

class AiFeaturesSettingControllerTest {
    @Test
    fun initialStateReflectsFlag() {
        assertEquals(AiFeaturesSettingState(enabled = true), Harness(flag = true).controller.state.value)
        assertEquals(AiFeaturesSettingState(enabled = false), Harness(flag = false).controller.state.value)
    }

    @Test
    fun turningOnShowsDisclosureAndStaysOffUntilAccepted() =
        runTest {
            val h = Harness()
            h.controller.turnOn()
            assertEquals(AiFeaturesConsentState(), h.gate.consent.value)
            assertFalse(h.controller.state.value.enabled)
            assertEquals(0, h.enableCalls)

            h.gate.turnOn()
            assertEquals(1, h.enableCalls)
            assertTrue(h.controller.state.value.enabled)
            assertNull(h.gate.consent.value)
        }

    @Test
    fun turningOnShowsDisclosureEvenWhenFlagIsAlreadyOn() {
        val h = Harness(flag = true)
        h.controller.turnOn()
        assertEquals(AiFeaturesConsentState(), h.gate.consent.value)
    }

    @Test
    fun notNowLeavesSwitchOffWithoutSaving() {
        val h = Harness()
        h.controller.turnOn()
        h.gate.notNow()
        assertFalse(h.controller.state.value.enabled)
        assertFalse(h.flag)
        assertEquals(0, h.enableCalls)
    }

    @Test
    fun turningOffSavesImmediatelyWithoutDisclosure() =
        runTest {
            val h = Harness(flag = true)
            h.controller.turnOff()
            assertEquals(1, h.disableCalls)
            assertFalse(h.flag)
            assertNull(h.gate.consent.value)
            assertEquals(AiFeaturesSettingState(enabled = false), h.controller.state.value)
        }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun switchIsOffAndDisabledWhileSaving() =
        runTest {
            val h = Harness(flag = true)
            h.disableGate = CompletableDeferred()
            launch { h.controller.turnOff() }
            runCurrent()
            assertEquals(AiFeaturesSettingState(enabled = false, saving = true), h.controller.state.value)

            h.controller.refresh()
            assertEquals(AiFeaturesSettingState(enabled = false, saving = true), h.controller.state.value)

            h.disableGate?.complete(Unit)
            runCurrent()
            assertEquals(AiFeaturesSettingState(enabled = false), h.controller.state.value)
        }

    @Test
    fun failedTurnOffRevertsSwitchAndShowsError() =
        runTest {
            val h = Harness(flag = true, disableFailure = ApiException.Network("offline"))
            h.controller.turnOff()
            assertTrue(h.flag)
            assertEquals(
                AiFeaturesSettingState(enabled = true, errorMessage = "offline"),
                h.controller.state.value,
            )
        }

    @Test
    fun turningOnAgainClearsPreviousError() =
        runTest {
            val h = Harness(flag = true, disableFailure = ApiException.Network("offline"))
            h.controller.turnOff()
            h.flag = false
            h.controller.refresh()
            h.controller.turnOn()
            assertNull(h.controller.state.value.errorMessage)
        }

    @Test
    fun refreshPicksUpFlagChangedElsewhere() {
        val h = Harness(flag = true)
        h.flag = false
        h.controller.refresh()
        assertFalse(h.controller.state.value.enabled)
        h.flag = true
        h.controller.refresh()
        assertTrue(h.controller.state.value.enabled)
    }
}
