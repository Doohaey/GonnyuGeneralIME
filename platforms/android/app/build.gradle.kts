import java.util.Base64

plugins {
    id("com.android.application")
    kotlin("android")
}

val workspaceManifest = rootProject.file("../../Cargo.toml").readText()
val productVersion = Regex("""(?ms)^\[workspace\.package]\s.*?^version\s*=\s*"([^"]+)"""")
    .find(workspaceManifest)
    ?.groupValues
    ?.get(1)
    ?: error("Missing workspace package version")
val versionParts = productVersion.split(".").map(String::toInt)
require(versionParts.size == 3)
val productVersionCode = versionParts[0] * 1_000_000 + versionParts[1] * 1_000 + versionParts[2]
val releaseKeystoreBase64 = System.getenv("ANDROID_KEYSTORE_BASE64")
val releaseKeystorePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
val releaseKeyAlias = System.getenv("ANDROID_KEY_ALIAS")
val releaseKeyPassword = System.getenv("ANDROID_KEY_PASSWORD")
val hasReleaseSigning = listOf(
    releaseKeystoreBase64,
    releaseKeystorePassword,
    releaseKeyAlias,
    releaseKeyPassword,
).all { !it.isNullOrBlank() }
val isReleaseRequested = gradle.startParameter.taskNames.any { it.contains("release", ignoreCase = true) }
require(hasReleaseSigning || !isReleaseRequested) {
    "Release signing requires ANDROID_KEYSTORE_BASE64, ANDROID_KEYSTORE_PASSWORD, ANDROID_KEY_ALIAS, and ANDROID_KEY_PASSWORD"
}

android {
    namespace = "io.gannyu.input"
    compileSdk = 34

    defaultConfig {
        applicationId = "io.gannyu.input"
        minSdk = 24
        targetSdk = 34
        versionCode = productVersionCode
        versionName = productVersion
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    if (hasReleaseSigning) {
        val keystoreFile = layout.buildDirectory.file("release.keystore").get().asFile
        keystoreFile.parentFile.mkdirs()
        keystoreFile.writeBytes(Base64.getDecoder().decode(releaseKeystoreBase64!!))
        signingConfigs {
            create("release") {
                storeFile = keystoreFile
                storePassword = releaseKeystorePassword!!
                keyAlias = releaseKeyAlias!!
                keyPassword = releaseKeyPassword!!
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = if (hasReleaseSigning) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    packaging {
        jniLibs {
            keepDebugSymbols += "**/*.so"
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
}
