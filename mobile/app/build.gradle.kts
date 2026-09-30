import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// Kunci RILIS dibaca dari mobile/keystore.properties — file itu & keystore-nya
// TIDAK PERNAH masuk git (lihat .gitignore & mobile/DESIGN.md bagian 11).
// Tidak ada -> APK rilis ditandatangani kunci DEBUG: tetap bisa dipasang untuk
// uji, TIDAK untuk Play Store.
val releaseKey: Properties? = rootProject.file("keystore.properties").takeIf { it.exists() }?.let { file ->
    Properties().apply { file.inputStream().use(::load) }
}

android {
    // Namespace kode (package Kotlin & kelas R) — SENGAJA beda dari
    // applicationId supaya mengganti identitas rilis tidak ikut
    // memindahkan package kode.
    namespace = "org.terminus.mobile"
    compileSdk = 37

    defaultConfig {
        // Identitas aplikasi (Play Store, instalasi di HP). SATU-SATUNYA tempat
        // ditulis (mobile/DESIGN.md bagian 2). Mengubahnya lagi = app BARU di
        // HP & Play Store (data & update tidak menyambung ke yang lama).
        applicationId = "com.ustdev.terminus"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // Tiap test instrumentasi mulai dari data app BERSIH (tanpa token /
        // known_hosts sisa test sebelumnya) — butuh orchestrator (testOptions).
        testInstrumentationRunnerArguments["clearPackageData"] = "true"
    }

    testOptions {
        execution = "ANDROIDX_TEST_ORCHESTRATOR"
    }

    signingConfigs {
        if (releaseKey != null) {
            create("release") {
                storeFile = rootProject.file(releaseKey.getProperty("storeFile"))
                storePassword = releaseKey.getProperty("storePassword")
                keyAlias = releaseKey.getProperty("keyAlias")
                keyPassword = releaseKey.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // R8: buang kode tak terpakai + samarkan nama. Aturan keep untuk
            // library ber-reflection ada di proguard-rules.pro — APK rilis WAJIB
            // diuji ulang penuh (login, SSH, SFTP) tiap aturan/dependency berubah.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
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
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.okhttp)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.termux.terminal.emulator)
    implementation(libs.sshj)
    implementation(libs.bouncycastle.prov)
    implementation(libs.androidx.biometric)
    implementation(libs.androidx.fragment)

    testImplementation(libs.junit)
    testImplementation(libs.okhttp.mockwebserver)
    testImplementation(libs.kotlinx.coroutines.test)

    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.espresso.core)
    androidTestUtil(libs.androidx.test.orchestrator)
    androidTestUtil(libs.androidx.test.services)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
}
