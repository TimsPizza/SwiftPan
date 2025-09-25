import java.util.Properties

plugins {
  id("com.android.application")
  id("org.jetbrains.kotlin.android")
  id("rust")
}
// read tauri.properties (version)
val tauriProperties =
        Properties().apply {
          val propFile = file("tauri.properties")
          if (propFile.exists()) {
            propFile.inputStream().use { load(it) }
          }
        }

// read keystore.properties (signatures)
val keystoreProperties =
        Properties().apply {
          val propFile = rootProject.file("keystore.properties")
          if (propFile.exists()) {
            propFile.inputStream().use { load(it) }
          }
        }

android {
  namespace = "com.timspizza.swiftpan"
  compileSdk = 36

  defaultConfig {
    applicationId = "com.timspizza.swiftpan"
    minSdk = 24
    targetSdk = 36

    versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
    versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")

    manifestPlaceholders["usesCleartextTraffic"] = "false"
  }

  val haveKeystore = listOf("storeFile","password","keyAlias").all { keystoreProperties.getProperty(it)?.isNotBlank() == true }
  signingConfigs {
    if (haveKeystore) {
      create("release") {
        val storePath = keystoreProperties.getProperty("storeFile")
        storeFile = file(storePath)
        storePassword = keystoreProperties.getProperty("password")
        keyAlias = keystoreProperties.getProperty("keyAlias")
        keyPassword = keystoreProperties.getProperty("password") // same as store password unless separated

        enableV1Signing = false
        enableV2Signing = true
        enableV3Signing = true
      }
    }
  }

  buildTypes {
    getByName("debug") {
      manifestPlaceholders["usesCleartextTraffic"] = "true"
      isDebuggable = true
      isJniDebuggable = true
      isMinifyEnabled = false

      packaging { jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so") }
    }
    getByName("release") {
      if (haveKeystore) {
        signingConfig = signingConfigs.getByName("release")
      } else {
        println("[WARN] No keystore provided. Release variant will be unsigned (use debug for testing or supply secrets).")
      }
      isMinifyEnabled = true
      proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
    }
  }
  buildFeatures {
    buildConfig = true
  }
  // Java 17 toolchain (Android Gradle Plugin expects compileOptions, not a random kotlinOptions block here)
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }

}

rust {
    rootDirRel = "../../../"
}

// Kotlin JVM toolchain (ensures Gradle uses JDK 17 for Kotlin compilation even if host JDK differs)
kotlin {
  jvmToolchain(17)
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")

// Ensure Kotlin compiler targets JVM 17
tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
  kotlinOptions {
    jvmTarget = "17"
  }
}
