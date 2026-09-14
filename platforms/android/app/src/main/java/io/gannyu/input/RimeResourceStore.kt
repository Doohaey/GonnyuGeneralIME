package io.gannyu.input

import android.content.Context
import org.json.JSONObject
import java.io.File
import java.io.InputStream
import java.security.MessageDigest
import java.util.UUID

/** Installs the signed, versioned Rime bundle from APK assets into app storage. */
internal object RimeResourceStore {
    private const val ASSET_ROOT = "rime"
    private const val MANIFEST = "$ASSET_ROOT/resource-manifest.json"

    fun prepare(context: Context): File {
        val manifestText = context.assets.open(MANIFEST).bufferedReader().use { it.readText() }
        val manifest = JSONObject(manifestText)
        val schemaVersion = manifest.getString("schema_version").requireSafePathComponent("schema_version")
        val resourcesRoot = File(context.filesDir, "rime-resources")
        val target = File(resourcesRoot, schemaVersion)

        if (target.isDirectory && verify(target, manifest)) return target

        resourcesRoot.mkdirs()
        val staging = File(resourcesRoot, ".${schemaVersion}.staging-${UUID.randomUUID()}")
        staging.deleteRecursively()
        staging.mkdirs()
        try {
            copyBundle(context, staging, manifest)
            check(verify(staging, manifest)) { "Rime resource verification failed after asset copy" }
            if (target.exists()) target.deleteRecursively()
            check(staging.renameTo(target)) { "Could not atomically install Rime resources" }
            resourcesRoot.listFiles()?.forEach { entry ->
                if (entry != target) entry.deleteRecursively()
            }
            return target
        } catch (error: Throwable) {
            staging.deleteRecursively()
            throw error
        }
    }

    fun userDataDirectory(context: Context): File = File(context.filesDir, "rime-user").apply { mkdirs() }

    private fun copyBundle(context: Context, destination: File, manifest: JSONObject) {
        files(manifest).forEach { item ->
            val relativePath = item.getString("path").requireSafeRelativePath()
            val output = File(destination, relativePath)
            output.parentFile?.mkdirs()
            context.assets.open("$ASSET_ROOT/$relativePath").use { input ->
                output.outputStream().use(input::copyTo)
            }
        }
        File(destination, "resource-manifest.json").writeText(manifest.toString() + "\n")
    }

    private fun verify(root: File, manifest: JSONObject): Boolean = files(manifest).all { item ->
        val relativePath = item.getString("path").requireSafeRelativePath()
        val expectedSize = item.getLong("size")
        val expectedDigest = item.getString("sha256")
        val file = File(root, relativePath)
        file.isFile && file.length() == expectedSize && sha256(file.inputStream()) == expectedDigest
    }

    private fun files(manifest: JSONObject): List<JSONObject> {
        val list = manifest.getJSONArray("files")
        return List(list.length()) { index -> list.getJSONObject(index) }
    }

    private fun sha256(input: InputStream): String = input.use {
        val digest = MessageDigest.getInstance("SHA-256")
        val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
        while (true) {
            val count = it.read(buffer)
            if (count < 0) break
            digest.update(buffer, 0, count)
        }
        digest.digest().joinToString("") { byte -> "%02x".format(byte) }
    }

    private fun String.requireSafePathComponent(label: String): String {
        require(isNotBlank() && this !in setOf(".", "..") && !contains('/') && !contains('\\')) {
            "Invalid Rime $label"
        }
        return this
    }

    private fun String.requireSafeRelativePath(): String {
        require(isNotBlank() && !startsWith('/') && split('/').none { it.isEmpty() || it == "." || it == ".." }) {
            "Invalid Rime resource path"
        }
        return this
    }
}
