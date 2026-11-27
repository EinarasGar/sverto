package com.sverto.app

import org.junit.Assert.assertEquals
import org.junit.Test

class MainStartupModulesTest {
    @Test
    fun portfolio_starts_without_hidden_modules() {
        assertEquals(
            MainStartupModules(),
            mainStartupModules(currentRoute = "portfolio", listPaneRoute = null, hasSharedFiles = false),
        )
    }

    @Test
    fun transactions_starts_transaction_modules() {
        assertEquals(
            MainStartupModules(transactions = true, quickUploads = true),
            mainStartupModules(currentRoute = "transactions", listPaneRoute = null, hasSharedFiles = false),
        )
    }

    @Test
    fun myra_starts_only_chat_module() {
        assertEquals(
            MainStartupModules(aiChat = true),
            mainStartupModules(currentRoute = "ai_chat", listPaneRoute = null, hasSharedFiles = false),
        )
    }

    @Test
    fun transaction_detail_keeps_transaction_module_available() {
        assertEquals(
            MainStartupModules(transactions = true),
            mainStartupModules(
                currentRoute = "transactionDetail/{txId}",
                listPaneRoute = null,
                hasSharedFiles = false,
            ),
        )
    }

    @Test
    fun transactions_list_pane_keeps_its_modules_under_a_pushed_route() {
        assertEquals(
            MainStartupModules(transactions = true, quickUploads = true),
            mainStartupModules(currentRoute = "settings", listPaneRoute = "transactions", hasSharedFiles = false),
        )
    }

    @Test
    fun chat_list_pane_keeps_chat_module_under_a_pushed_route() {
        assertEquals(
            MainStartupModules(aiChat = true),
            mainStartupModules(currentRoute = "settings", listPaneRoute = "ai_chat", hasSharedFiles = false),
        )
    }

    @Test
    fun shared_files_start_transaction_modules() {
        assertEquals(
            MainStartupModules(transactions = true, quickUploads = true),
            mainStartupModules(currentRoute = "portfolio", listPaneRoute = null, hasSharedFiles = true),
        )
    }

    @Test
    fun cached_user_restores_onboarding_destination() {
        assertEquals(
            SignedInStartupState(userId = "user-id", onboarded = true),
            signedInStartupState(
                userId = "user-id",
                onboardingVersion = 1,
                currentOnboardingVersion = 1,
            ),
        )
    }
}
