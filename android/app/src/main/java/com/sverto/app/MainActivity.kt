package com.sverto.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.IntentCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sverto.app.core.theme.SvertoTheme
import com.sverto.app.feature.onboarding.CURRENT_ONBOARDING_VERSION
import com.sverto.app.feature.onboarding.OnboardingScreen
import com.sverto.app.feature.server.AppSessionViewModel
import com.sverto.app.feature.server.ClerkLoginScreen
import com.sverto.app.feature.server.ConnectErrorScreen
import com.sverto.app.feature.server.PasswordSignInScreen
import com.sverto.app.feature.server.PasswordSignUpScreen
import com.sverto.app.feature.server.ServerUrlScreen
import com.sverto.app.feature.server.SessionState
import com.sverto.app.feature.server.WelcomeScreen
import com.sverto.app.feature.update.UpdateRequiredScreen
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.sverto_core.AppStore

class MainActivity : ComponentActivity() {
    private val sharedImageUris: MutableState<List<Uri>> = mutableStateOf(emptyList())
    private val sessionViewModel: AppSessionViewModel by viewModels()
    private val appUpdates by lazy { (application as SvertoApp).appUpdates }
    private val updateFlowLauncher =
        registerForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) {}

    @OptIn(ExperimentalMaterial3ExpressiveApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        appUpdates.attach(updateFlowLauncher)
        if (savedInstanceState == null) {
            consumeSharedImages(intent)
        }
        setContent {
            SvertoTheme {
                val appStore = remember { (applicationContext as SvertoApp).appStore }
                val sharedImages = sharedImageUris.value
                val onSharedImagesHandled = { sharedImageUris.value = emptyList() }
                val sessionState by sessionViewModel.state.collectAsStateWithLifecycle()
                val currentSession = sessionState

                val updatePresentation = appUpdates.presentation.collectAsStateWithLifecycle()
                val updateBlocking by remember { derivedStateOf { updatePresentation.value.blocking } }
                LaunchedEffect(currentSession) {
                    appUpdates.setRestartSafe(
                        currentSession == SessionState.Welcome || currentSession is SessionState.SignIn,
                    )
                }

                if (updateBlocking) {
                    UpdateRequiredScreen(onUpdate = appUpdates::startRequiredUpdate)
                } else {
                    when (currentSession) {
                        SessionState.Loading -> LoadingScreen()
                        SessionState.Welcome ->
                            WelcomeScreen(
                                onContinueWithHost = { sessionViewModel.continueWithHosted() },
                                onConnectSelfHosted = { sessionViewModel.showServerUrlInput() },
                            )
                        is SessionState.ServerUrl ->
                            ServerUrlScreen(
                                lastUrl = currentSession.lastUrl,
                                connectionStatus = currentSession.connectionStatus,
                                onCheckServer = { url -> sessionViewModel.checkSelfHostedServer(url) },
                                onContinue = { sessionViewModel.continueWithSelfHosted() },
                                onBack = { sessionViewModel.showWelcome() },
                            )
                        SessionState.Connecting -> LoadingScreen()
                        is SessionState.ConnectError ->
                            ConnectErrorScreen(
                                message = currentSession.message,
                                onRetry = { sessionViewModel.retryHostedConnect() },
                                onBack = { sessionViewModel.showWelcome() },
                            )
                        is SessionState.SignIn ->
                            PasswordSignInScreen(
                                onSignIn = { u, p -> sessionViewModel.signIn(u, p) },
                                onCreateAccount = { sessionViewModel.showSignUp() },
                                onBack = { sessionViewModel.showWelcome() },
                                isSubmitting = currentSession.isSubmitting,
                                errorMessage = currentSession.errorMessage,
                            )
                        SessionState.SignUp ->
                            PasswordSignUpScreen(
                                onCreateAccount = { u, p -> sessionViewModel.signUp(u, p) },
                                onBack = { sessionViewModel.showSignIn() },
                            )
                        SessionState.ClerkLogin ->
                            ClerkLoginScreen(
                                onSignedIn = { sessionViewModel.onClerkSignedIn() },
                                onBack = { sessionViewModel.showWelcome() },
                            )
                        SessionState.SignedIn ->
                            SignedInGate(
                                appStore = appStore,
                                signInKey = Unit,
                                sharedImageUris = sharedImages,
                                onSharedImagesHandled = onSharedImagesHandled,
                                sessionViewModel = sessionViewModel,
                            )
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        consumeSharedImages(intent)
    }

    override fun onResume() {
        super.onResume()
        appUpdates.onActivityResumed()
    }

    override fun onDestroy() {
        appUpdates.detach(updateFlowLauncher)
        super.onDestroy()
    }

    @Suppress("OVERRIDE_DEPRECATION")
    override fun startActivityForResult(
        intent: Intent,
        requestCode: Int,
        options: Bundle?,
    ) {
        super.startActivityForResult(intent, requestCode, options)
        appUpdates.onLeavingForActivity()
    }

    private fun consumeSharedImages(intent: Intent?) {
        val uris = extractSharedImageUris(intent)
        if (uris.isNotEmpty()) {
            sharedImageUris.value = uris
            intent?.removeExtra(Intent.EXTRA_STREAM)
            intent?.action = Intent.ACTION_MAIN
        }
    }

    private fun extractSharedImageUris(intent: Intent?): List<Uri> {
        if (intent == null || intent.type?.startsWith("image/") != true) return emptyList()
        return when (intent.action) {
            Intent.ACTION_SEND ->
                IntentCompat
                    .getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
                    ?.let { listOf(it) }
                    ?: emptyList()
            Intent.ACTION_SEND_MULTIPLE ->
                IntentCompat
                    .getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
                    ?.filterNotNull()
                    ?: emptyList()
            else -> emptyList()
        }
    }
}

@Composable
private fun SignedInGate(
    appStore: AppStore,
    signInKey: Any?,
    sharedImageUris: List<Uri>,
    onSharedImagesHandled: () -> Unit,
    sessionViewModel: AppSessionViewModel,
) {
    var onboarded by remember(signInKey) { mutableStateOf<Boolean?>(null) }
    var onboardingViewModelKey by remember(signInKey) { mutableStateOf<String?>(null) }
    val context = LocalContext.current
    val appUpdates = remember { (context.applicationContext as SvertoApp).appUpdates }
    LaunchedEffect(onboarded) {
        if (onboarded != true) appUpdates.setRestartSafe(false)
    }
    LaunchedEffect(signInKey) {
        val serverUrl = sessionViewModel.activeServerUrl.value.orEmpty()
        val cachedMe = withContext(Dispatchers.IO) { appStore.restoreCachedMe() }
        if (cachedMe != null) {
            val startupState =
                signedInStartupState(
                    userId = cachedMe.userId,
                    onboardingVersion = cachedMe.onboardingVersion,
                    currentOnboardingVersion = CURRENT_ONBOARDING_VERSION,
                )
            onboardingViewModelKey = "$serverUrl:${startupState.userId}"
            onboarded = startupState.onboarded
        }

        appStore.onSignIn()
        val refreshedMe = withContext(Dispatchers.IO) { appStore.getCachedMe() }
        val startupState =
            if (refreshedMe != null) {
                signedInStartupState(
                    userId = refreshedMe.userId,
                    onboardingVersion = refreshedMe.onboardingVersion,
                    currentOnboardingVersion = CURRENT_ONBOARDING_VERSION,
                )
            } else {
                withContext(Dispatchers.IO) {
                    signedInStartupState(
                        userId = "anonymous",
                        onboardingVersion = appStore.getOnboardingVersion(),
                        currentOnboardingVersion = CURRENT_ONBOARDING_VERSION,
                    )
                }
            }
        onboardingViewModelKey = "$serverUrl:${startupState.userId}"
        onboarded = startupState.onboarded
    }
    when (onboarded) {
        null -> LoadingScreen()
        true ->
            MainScreen(
                sharedImageUris = sharedImageUris,
                onSharedImagesHandled = onSharedImagesHandled,
                sessionViewModel = sessionViewModel,
            )
        false ->
            OnboardingScreen(
                onComplete = { onboarded = true },
                viewModelKey = onboardingViewModelKey,
            )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LoadingScreen() {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        LoadingIndicator()
    }
}
