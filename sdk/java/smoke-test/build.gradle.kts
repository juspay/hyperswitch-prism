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
    implementation("com.google.protobuf:protobuf-java:4.33.4")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.0")
    // Depend on published SDK to avoid Gradle circular dependency (root :jar -> :classes -> :compileKotlin).
    // CI and Makefile run publishToMavenLocal (or equivalent) before running smoke-test.
    implementation("com.hyperswitch:payments-client:0.1.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
}

sourceSets {
    main {
        // Include generated connector examples so process* functions are available via reflection
        // for the FFI smoke test (test-package target). Examples are not needed for gRPC test.
        kotlin.srcDir(file("../../../examples/stripe"))
        resources.srcDir(file("src/main/resources"))
    }
}

application {
    mainClass.set("SmokeTestKt")
}

// Configure processResources to handle duplicates
tasks.processResources {
    duplicatesStrategy = DuplicatesStrategy.INCLUDE
}

// Task to run the gRPC smoke test
tasks.register<JavaExec>("runGrpc") {
    group = "application"
    description = "Run the gRPC smoke test"
    classpath = sourceSets["main"].runtimeClasspath
    mainClass.set("GrpcSmokeTestKt")

    // Force ANSI color output even when stdout is piped (e.g. through `make | tail`)
    environment("FORCE_COLOR", "1")

    // Suppress JNA "restricted method" warning (Java 17+) and protobuf Unsafe warning (Java 21+)
    jvmArgs(
        "--enable-native-access=ALL-UNNAMED",
        "--sun-misc-unsafe-memory-access=allow",
    )

    // Pass through all project properties as system properties
    systemProperty("jna.library.path", file("../src/main/resources/native").absolutePath)
    systemProperty("hyperswitch.grpc.lib.path",
        file("src/main/resources/native/libhyperswitch_grpc_ffi.dylib").absolutePath)

    // Forward any args passed to this task
    args = project.properties["args"]?.toString()?.split(" ") ?: emptyList()
}

tasks.named<JavaExec>("run") {
    systemProperty("jna.library.path",
        file("../src/main/resources/native").absolutePath)
}
