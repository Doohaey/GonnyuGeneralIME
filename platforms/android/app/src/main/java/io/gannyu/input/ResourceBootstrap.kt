package io.gannyu.input

import android.content.Context
import java.io.File

object ResourceBootstrap {
    fun ensureResources(context: Context): File = File(context.filesDir, "resources")
}
