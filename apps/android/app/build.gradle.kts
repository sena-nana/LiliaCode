plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.lilia.remote"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.lilia.remote"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-alpha.1"
    }

    ndkVersion = "28.2.13676358"

    buildFeatures {
        compose = true
    }

    packaging {
        resources {
            excludes.addAll(
                listOf(
                    "aix-*/**",
                    "darwin-*/**",
                    "freebsd-*/**",
                    "linux-*/**",
                    "openbsd-*/**",
                    "sunos-*/**",
                    "win32-*/**",
                    "com/sun/jna/aix-*/**",
                    "com/sun/jna/darwin-*/**",
                    "com/sun/jna/freebsd-*/**",
                    "com/sun/jna/linux-*/**",
                    "com/sun/jna/openbsd-*/**",
                    "com/sun/jna/sunos-*/**",
                    "com/sun/jna/win32-*/**",
                ),
            )
        }
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.05.00"))
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("computer.iroh:iroh:1.0.0")

    debugImplementation("androidx.compose.ui:ui-tooling")
}
