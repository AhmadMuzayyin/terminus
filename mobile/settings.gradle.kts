pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

plugins {
    // Mesin dev boleh cuma punya JRE (tanpa javac) — JDK 21 buat compile
    // diunduh otomatis Gradle ke ~/.gradle/jdks, tidak mengubah sistem.
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // Library terminal Termux (Milestone 4) cuma dipublikasikan di JitPack.
        maven("https://jitpack.io")
    }
}

rootProject.name = "terminus-mobile"
include(":app")
