plugins {
    kotlin("jvm") version "2.3.10"
    application
}

repositories {
    mavenLocal()
    mavenCentral()
}

dependencies {
    implementation("com.google.code.gson:gson:2.10.1")
    // Depend on published SDK to avoid Gradle circular dependency (root :jar -> :classes -> :compileKotlin).
    // CI and Makefile run publishToMavenLocal (or equivalent) before running smoke-test.
    implementation("com.hyperswitch:payments-client:0.1.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
}

application {
    mainClass.set("SmokeTestKt")
}

tasks.named<JavaExec>("run") {
    systemProperty("jna.library.path",
        file("../src/main/resources/native").absolutePath)
}
