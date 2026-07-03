plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.lilia.voice"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.lilia.voice"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "1.0.0-beta"
    }

    ndkVersion = "28.2.13676358"

    buildFeatures {
        compose = true
    }
}

dependencies {
    implementation(project(":remote-core"))
    implementation(platform("androidx.compose:compose-bom:2026.05.00"))
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    compileOnly("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    testImplementation("junit:junit:4.13.2")
}
