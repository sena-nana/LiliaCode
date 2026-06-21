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

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
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
    implementation("androidx.camera:camera-camera2:1.4.2")
    implementation("androidx.camera:camera-lifecycle:1.4.2")
    implementation("androidx.camera:camera-view:1.4.2")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("com.google.mlkit:barcode-scanning:17.3.0")
    implementation("computer.iroh:iroh:1.0.0")

    compileOnly("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}
