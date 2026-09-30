// Plugin dideklarasikan di sini (apply false) supaya versinya satu
// sumber (gradle/libs.versions.toml), dipakai di app/build.gradle.kts.
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.kotlin.serialization) apply false
}
