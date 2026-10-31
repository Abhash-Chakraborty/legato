plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "tech.abhashchakraborty.legato"
    compileSdk = 35

    defaultConfig {
        applicationId = "tech.abhashchakraborty.legato"
        minSdk = 29 // Android 10: the clipboard rules the design is built around start here
        targetSdk = 35
        versionCode = 4
        versionName = "0.3.1"
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    // CI hands over one key for all test builds (see android.yml), so each
    // APK updates the last instead of needing an uninstall.
    val keystore = System.getenv("LEGATO_KEYSTORE")?.let { file(it) }?.takeIf { it.exists() }
    signingConfigs {
        if (keystore != null) {
            create("legato") {
                storeFile = keystore
                storeType = "pkcs12"
                storePassword = System.getenv("LEGATO_KEYSTORE_PASSWORD")
                keyAlias = "legato"
                keyPassword = System.getenv("LEGATO_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        debug {
            if (keystore != null) signingConfig = signingConfigs.getByName("legato")
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true; buildConfig = true }
    sourceSets {
        // The Kotlin bindings UniFFI generates from the Rust core (see .github/workflows/android.yml).
        getByName("main").java.srcDirs("build/generated/uniffi")
    }
    packaging {
        jniLibs { useLegacyPackaging = false }
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.navigation:navigation-compose:2.8.5")
    // Scanning the PC's pairing code: CameraX for the preview, ZXing to read it (no Play Services).
    implementation("androidx.camera:camera-camera2:1.4.1")
    implementation("androidx.camera:camera-lifecycle:1.4.1")
    implementation("androidx.camera:camera-view:1.4.1")
    implementation("com.google.zxing:core:3.5.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    // Springy motion for the bubble (Material 3 Expressive feel without Compose in the overlay).
    // UniFFI's Kotlin bindings call the Rust library through JNA.
    implementation("net.java.dev.jna:jna:5.15.0@aar")
    debugImplementation("androidx.compose.ui:ui-tooling")
}
