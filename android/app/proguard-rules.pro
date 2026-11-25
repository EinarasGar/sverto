# Keep JNA classes used by UniFFI bindings
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn java.awt.**

# Keep UniFFI generated bindings
-keep class uniffi.** { *; }

# ML Kit GenAI (beta) ships incomplete consumer rules: its component registrars
# are instantiated reflectively from manifest meta-data, but R8 strips their
# no-arg constructors, making Generation.getClient() throw NPE in release builds.
-keep class * implements com.google.firebase.components.ComponentRegistrar { <init>(); }

# Lucide icon picker enumerates icons by reflecting over the library's R drawable
# class and resolves them by name via Resources.getIdentifier (core/icons/LucideIcon.kt).
# R8 can't see these dynamic references, so without this the fields are stripped and
# the picker is empty in release builds.
-keep class com.composables.icons.lucide.R$drawable { *; }

# OkHttp
-dontwarn okhttp3.internal.**

# Clerk SDK. Its bundled rules also keep all of Compose runtime, Material 3, Coil, Play services,
# Credentials and every ViewModel, which blocks R8 on most of the app. build.gradle.kts ignores
# them, and the rest of this file repeats them without those library-wide keeps.
-keep class com.clerk.** { *; }
-dontwarn com.clerk.**
-keep class com.auth0.android.jwt.** { *; }
-keepclassmembers class * extends androidx.lifecycle.ViewModel {
    <init>(...);
}

-keepattributes *Annotation*, InnerClasses, Signature, Exceptions
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class kotlinx.serialization.json.** {
    *** Companion;
}
-keepclasseswithmembers class kotlinx.serialization.json.** {
    kotlinx.serialization.KSerializer serializer(...);
}
-keepclassmembers class * {
    @kotlinx.serialization.Serializable <fields>;
}

-keepclassmembers,allowshrinking,allowobfuscation interface * {
    @retrofit2.http.* <methods>;
}
-if interface * { @retrofit2.http.* <methods>; }
-keep,allowobfuscation interface <1>
-if interface * { @retrofit2.http.* <methods>; }
-keep,allowobfuscation interface * extends <1>
-keep,allowobfuscation,allowshrinking class kotlin.coroutines.Continuation
-dontwarn org.codehaus.mojo.animal_sniffer.IgnoreJRERequirement
-dontwarn javax.annotation.**
-dontwarn kotlin.Unit
-dontwarn retrofit2.KotlinExtensions
-dontwarn retrofit2.KotlinExtensions$*

-dontwarn okhttp3.internal.platform.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**

-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}
-keepclassmembers class kotlinx.coroutines.** {
    volatile <fields>;
}
-keepclassmembers class kotlin.coroutines.SafeContinuation {
    volatile <fields>;
}
-dontwarn java.lang.invoke.StringConcatFactory
-dontwarn androidx.compose.runtime.**
-dontwarn coil3.**
