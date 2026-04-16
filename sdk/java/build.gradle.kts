plugins {
    kotlin("jvm") version "2.3.10"
    `java-library`
    `maven-publish`
    signing
    id("com.tddworks.central-publisher") version "0.2.0-alpha.1"
}

group = "io.hyperswitch"
version = "0.0.5"

repositories {
    mavenCentral()
}

dependencies {
    // api = exposed to consumers at compile time (published as compile scope in POM)
    // Version must match protoc (protoc --version → libprotoc X.Y → protobuf-java 4.X.Y)
    api("com.google.protobuf:protobuf-java:4.33.4")
    // JNA required by UniFFI-generated Kotlin bindings (exposed in public API)
    api("net.java.dev.jna:jna:5.14.0")
    api("com.google.code.gson:gson:2.11.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.json:json:20240303")
}

// Create a separate source set for the sanity runner
sourceSets {
    create("sanity") {
        kotlin.srcDir("tests")
        compileClasspath += sourceSets["main"].output + sourceSets["main"].compileClasspath
        runtimeClasspath += sourceSets["main"].output + sourceSets["main"].compileClasspath
    }
}

// Compile the sanity runner
tasks.named<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>("compileSanityKotlin") {
    dependsOn("compileKotlin")
}

tasks.register<JavaExec>("runClientSanity") {
    group = "verification"
    description = "Run client sanity certification runner"
    mainClass.set("ClientSanityRunnerKt")
    classpath = sourceSets["sanity"].runtimeClasspath
    standardInput = System.`in`
    systemProperty("jna.library.path",
        file("src/main/resources/native").absolutePath)
    dependsOn("compileSanityKotlin")
}

// Signing configuration - required for Maven Central
// Local Maven (publishToMavenLocal) doesn't require signing
val signingKey = System.getenv("GPG_SIGNING_KEY")
val signingPassword = System.getenv("GPG_SIGNING_KEY_PASSWORD")
val hasSigningCredentials = !signingKey.isNullOrBlank() && !signingPassword.isNullOrBlank()

// Pre-configure signing with keys (actual signing setup deferred to afterEvaluate)
if (hasSigningCredentials) {
    signing {
        useInMemoryPgpKeys(signingKey, signingPassword)
    }
}
// When no credentials: no signing configuration, allowing local publishToMavenLocal to work

// Configure Central Portal Publisher plugin
// Only configure if credentials are present (avoids validation errors during regular builds)
if (System.getenv("CENTRAL_TOKEN_USERNAME") != null) {
    centralPublisher {
        credentials {
            username = System.getenv("CENTRAL_TOKEN_USERNAME") ?: ""
            password = System.getenv("CENTRAL_TOKEN_PASSWORD") ?: ""
        }

        projectInfo {
            name = "Hyperswitch Prism"
            description = "Hyperswitch Payments SDK - Kotlin client for connector integrations"
            url = "https://github.com/juspay/hyperswitch-prism"

            license {
                name = "MIT License"
                url = "https://opensource.org/licenses/MIT"
            }

            developer {
                id = "juspay"
                name = "Juspay"
                email = "hyperswitch@juspay.in"
            }

            scm {
                url = "https://github.com/juspay/hyperswitch-prism"
                connection = "scm:git:git://github.com/juspay/hyperswitch-prism.git"
                developerConnection = "scm:git:ssh://github.com/juspay/hyperswitch-prism.git"
            }
        }

        publishing {
            autoPublish = true
            aggregation = true
            dryRun = false
        }
    }
}

// Standard publishing configuration for local Maven (publishToMavenLocal)
// This runs alongside the central-publisher plugin which handles Maven Central
publishing {
    publications {
        create<MavenPublication>("mavenLocal") {
            from(components["java"])
            artifactId = "prism"

            pom {
                name.set("Hyperswitch Prism")
                description.set("Hyperswitch Payments SDK - Kotlin client for connector integrations")
                url.set("https://github.com/juspay/hyperswitch-prism")

                licenses {
                    license {
                        name.set("MIT License")
                        url.set("https://opensource.org/licenses/MIT")
                    }
                }

                developers {
                    developer {
                        id.set("juspay")
                        name.set("Juspay")
                        email.set("hyperswitch@juspay.in")
                    }
                }

                scm {
                    url.set("https://github.com/juspay/hyperswitch-prism")
                    connection.set("scm:git:git://github.com/juspay/hyperswitch-prism.git")
                    developerConnection.set("scm:git:ssh://github.com/juspay/hyperswitch-prism.git")
                }
            }
        }
    }
}

// Note: The Central Publisher plugin automatically generates sources and javadoc jars

// Sign only the central-publisher's publication (named "maven") for Maven Central
// The "mavenLocal" publication is for local use and doesn't require signing
afterEvaluate {
    if (hasSigningCredentials) {
        // Find the central-publisher's publication and sign only that one
        publishing.publications.findByName("maven")?.let { pub ->
            signing.sign(pub)
        }
    }
}
