plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    // Namespace kode (package Kotlin & kelas R) — SENGAJA beda dari
    // applicationId supaya mengganti identitas rilis tidak ikut
    // memindahkan package kode.
    namespace = "org.terminus.mobile"
    compileSdk = 37

    defaultConfig {
        // PLACEHOLDER — diganti user sebelum rilis Play Store. SATU-SATUNYA
        // tempat identitas aplikasi ditulis (mobile/DESIGN.md bagian 2).
        applicationId = "com.example.terminus"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }
}

// JDK yang dipakai compile (bukan JDK yang menjalankan Gradle) — lihat
// foojay resolver di settings.gradle.kts.
java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    debugImplementation(libs.androidx.compose.ui.tooling)

    testImplementation(libs.junit)
}
