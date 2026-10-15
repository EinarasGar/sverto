package com.sverto.app

import android.app.Application
import android.net.ConnectivityManager
import android.net.Network
import com.clerk.api.Clerk
import com.clerk.api.network.serialization.ClerkResult
import com.sverto.app.core.ai.CategorySuggester
import com.sverto.app.core.update.AppUpdateController
import com.sverto.app.core.update.UpdateChannelSelector
import com.sverto.app.core.update.UpdatePreferences
import com.sverto.app.feature.server.KeystoreCredentialStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.AuthProvider
import java.util.concurrent.atomic.AtomicBoolean

class SvertoAuthProvider(
    private val hasConnectivity: () -> Boolean,
) : AuthProvider {
    @Volatile
    private var cachedToken: String? = null

    override suspend fun getToken(): String? {
        if (BuildConfig.CLERK_PUBLISHABLE_KEY.isBlank()) return null
        if (!hasConnectivity()) return cachedToken

        val token =
            when (val result = Clerk.auth.getToken()) {
                is ClerkResult.Success -> result.value
                is ClerkResult.Failure -> null
            }

        cachedToken = token
        return token
    }

    override fun getUserId(): String? {
        if (BuildConfig.CLERK_PUBLISHABLE_KEY.isBlank()) return null
        return Clerk.user?.id
    }
}

class SvertoApp : Application() {
    private val clerkInitializationStarted = AtomicBoolean(false)
    private val connectivityScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    val categorySuggester by lazy { CategorySuggester(this) }
    val appUpdates by lazy { AppUpdateController(UpdateChannelSelector.select(this), UpdatePreferences(this)) }

    lateinit var appStore: AppStore
        private set

    override fun onCreate() {
        super.onCreate()
        ensureClerkInitialised()
        System.loadLibrary("jnidispatch")
        System.loadLibrary("sverto_core")

        val dbPath = "${filesDir.absolutePath}/sverto_cache.db"

        appStore =
            AppStore(
                BuildConfig.API_BASE_URL,
                60u,
                dbPath,
                SvertoAuthProvider(::hasConnectivity),
                KeystoreCredentialStore(this),
            )

        registerConnectivityCallback()
    }

    fun ensureClerkInitialised() {
        val key = BuildConfig.CLERK_PUBLISHABLE_KEY
        if (key.isNotBlank() && clerkInitializationStarted.compareAndSet(false, true)) {
            Clerk.initialize(this, publishableKey = key)
        }
    }

    fun hasConnectivity(): Boolean = getSystemService(ConnectivityManager::class.java).activeNetwork != null

    private fun registerConnectivityCallback() {
        val cm = getSystemService(ConnectivityManager::class.java)
        val connected = hasConnectivity()
        val wasConnected = AtomicBoolean(connected)
        appStore.setConnectivity(connected)
        cm.registerDefaultNetworkCallback(
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    val isReconnect = !wasConnected.getAndSet(true)
                    appStore.setConnectivity(true)
                    if (isReconnect) {
                        connectivityScope.launch { appStore.refreshQuickUploads() }
                    }
                }

                override fun onLost(network: Network) {
                    val stillConnected = hasConnectivity()
                    wasConnected.set(stillConnected)
                    appStore.setConnectivity(stillConnected)
                }
            },
        )
    }
}
