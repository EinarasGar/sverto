package com.sverto.app.feature.settings

import com.sverto.app.core.ai.OnDeviceAiConsent
import com.sverto.app.core.ai.OnDeviceAiSettings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

private class OnDeviceAiHarness {
    val saved = mutableListOf<Boolean>()
    val controller = OnDeviceAiSettingController(setEnabled = { saved += it })
}

class OnDeviceAiSettingControllerTest {
    @Test
    fun turningOnShowsDisclosureWithoutEnabling() {
        val h = OnDeviceAiHarness()
        h.controller.turnOn()
        assertTrue(h.controller.consentVisible.value)
        assertEquals(emptyList<Boolean>(), h.saved)
    }

    @Test
    fun turnOnInDisclosureEnablesAndClosesOnceEnabled() {
        val h = OnDeviceAiHarness()
        h.controller.turnOn()
        h.controller.confirmTurnOn()
        assertEquals(listOf(true), h.saved)

        h.controller.onSettingsChanged(OnDeviceAiSettings(consent = OnDeviceAiConsent.DISABLED, saving = true))
        assertTrue(h.controller.consentVisible.value)

        h.controller.onSettingsChanged(OnDeviceAiSettings(consent = OnDeviceAiConsent.ENABLED))
        assertFalse(h.controller.consentVisible.value)
    }

    @Test
    fun failedEnableKeepsDisclosureOpenForTheError() {
        val h = OnDeviceAiHarness()
        h.controller.turnOn()
        h.controller.confirmTurnOn()
        h.controller.onSettingsChanged(
            OnDeviceAiSettings(consent = OnDeviceAiConsent.DISABLED, errorMessage = "Could not save your choice."),
        )
        assertTrue(h.controller.consentVisible.value)
    }

    @Test
    fun notNowClosesDisclosureAndStaysOff() {
        val h = OnDeviceAiHarness()
        h.controller.turnOn()
        h.controller.notNow()
        assertFalse(h.controller.consentVisible.value)
        assertEquals(emptyList<Boolean>(), h.saved)
    }

    @Test
    fun turningOffSavesImmediatelyWithoutDisclosure() {
        val h = OnDeviceAiHarness()
        h.controller.turnOff()
        assertEquals(listOf(false), h.saved)
        assertFalse(h.controller.consentVisible.value)
    }
}
