package io.gannyu.input

import android.content.Context
import android.inputmethodservice.InputMethodService
import android.util.Log
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject

private class NativePipelineBridge {
    init { System.loadLibrary("gannyu_input_jni") }

    external fun nativeCreate(manifestPath: String?, regionId: String?, dataDir: String): Long
    external fun nativeLastError(): String?
    external fun nativeRegionList(manifestPath: String?): String?
    external fun nativeUserDataClear(handle: Long, scope: Int): Boolean
    external fun nativeDestroy(handle: Long)
}

class GannyuInputMethodService : InputMethodService() {
    private data class RankedCandidate(
        val text: String,
        val comment: String?,
        val consumedBytes: Int,
        val reading: String?,
        val mandarinReading: String?,
        val mandarinOnly: Boolean = false
    )

    data class RegionOption(val id: String, val nameZh: String) {
        val displayLabel: String
            get() = "$nameZh（$id）"
    }

    private var pipelineHandle: Long = 0
    private var pipelineReady: Boolean = false
    private lateinit var preeditView: TextView
    private lateinit var candidateScroll: HorizontalScrollView
    private lateinit var candidateBar: LinearLayout
    private lateinit var keyboardRows: LinearLayout
    private val composing = StringBuilder()
    private val accumulatedText = StringBuilder()
    private val accumulatedReading = mutableListOf<String>()
    private val accumulatedMandarinReading = mutableListOf<String>()
    private var lastCandidates: List<RankedCandidate> = emptyList()
    private var symbolPage = false

    external fun nativeCreate(manifestPath: String?, regionId: String?, dataDir: String): Long
    external fun nativeLastError(): String?
    external fun nativeRegionList(manifestPath: String?): String?
    external fun nativeRetrieve(handle: Long, input: String): String?
    external fun nativeFormatPreedit(handle: Long, input: String, consumedBytes: Int): String?
    external fun nativeUserDictAdd(handle: Long, headword: String, pinyin: String, mandarinPinyin: String): Boolean
    external fun nativeUserDictBoost(handle: Long, headword: String): Boolean
    external fun nativeUserDataClear(handle: Long, scope: Int): Boolean
    external fun nativeDestroy(handle: Long)
    external fun nativeEntryCount(handle: Long): Int

    companion object {
        private const val TAG = "GannyuIME"
        private const val PREFS_NAME = "gannyu.runtime"
        private const val KEY_SELECTED_REGION = "selected_region"
        const val USER_DATA_WORDS = 1
        const val USER_DATA_FREQUENCIES = 2
        const val USER_DATA_ALL = 3
        @Volatile var preloadedHandle: Long = 0
        @JvmField val preloadLock = Object()
        @Volatile private var staticHandle: Long = 0
        @Volatile private var staticRegionId: String? = null
        @Volatile private var preloadedRegionId: String? = null

        @JvmStatic
        fun nativeCreateStatic(manifest: String?, region: String?, dataDir: String): Long {
            return nativeBridge.nativeCreate(manifest, region, dataDir)
        }

        @JvmStatic
        fun nativeRegionListStatic(manifest: String?): String? {
            return nativeBridge.nativeRegionList(manifest)
        }


        @JvmStatic
        fun nativeLastErrorStatic(): String? = nativeBridge.nativeLastError()

        @JvmStatic
        fun selectedRegionId(context: Context): String? =
            normalizeRegionId(
                context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                    .getString(KEY_SELECTED_REGION, null)
            )

        @JvmStatic
        fun setSelectedRegionId(context: Context, regionId: String?) {
            context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                .edit()
                .putString(KEY_SELECTED_REGION, normalizeRegionId(regionId))
                .apply()
        }

        @JvmStatic
        fun availableRegions(): List<RegionOption> = parseRegionList(nativeRegionListStatic(null))

        @JvmStatic
        fun clearUserDataAsync(context: Context, scope: Int, onComplete: (Boolean) -> Unit) {
            Thread {
                val success = synchronized(preloadLock) {
                    val handles = linkedSetOf<Long>()
                    if (staticHandle != 0L) handles += staticHandle
                    if (preloadedHandle != 0L) handles += preloadedHandle
                    if (handles.isNotEmpty()) {
                        handles.all { nativeBridge.nativeUserDataClear(it, scope) }
                    } else {
                        val region = selectedRegionId(context)
                        val handle = nativeCreateStatic(null, region, context.filesDir.absolutePath)
                        if (handle == 0L) false else {
                            val cleared = nativeBridge.nativeUserDataClear(handle, scope)
                            if (cleared) {
                                preloadedHandle = handle
                                preloadedRegionId = normalizeRegionId(region)
                            } else {
                                destroyHandle(handle)
                            }
                            cleared
                        }
                    }
                }
                onComplete(success)
            }.start()
        }

        @JvmStatic
        fun preloadSelectedRegionAsync(
            context: Context,
            regionId: String? = selectedRegionId(context),
            onComplete: ((Boolean, String?) -> Unit)? = null,
        ) {
            val desiredRegion = normalizeRegionId(regionId)
            Thread {
                try {
                    val handle = nativeCreateStatic(null, desiredRegion, context.filesDir.absolutePath)
                    val errorDetail = if (handle == 0L) nativeLastErrorStatic() else null
                    Log.i(TAG, "preload handle=" + handle)
                    if (handle != 0L) {
                        synchronized(preloadLock) {
                            if (preloadedHandle != 0L) {
                                destroyHandle(preloadedHandle)
                            }
                            preloadedHandle = handle
                            preloadedRegionId = desiredRegion
                            preloadLock.notifyAll()
                        }
                        onComplete?.invoke(true, null)
                        return@Thread
                    }
                    onComplete?.invoke(false, errorDetail.orEmpty().ifBlank { "native pipeline creation returned no detail" })
                } catch (e: Exception) {
                    Log.e(TAG, "preload failed", e)
                    onComplete?.invoke(false, e.stackTraceToString())
                }
            }.start()
        }

        @JvmStatic
        fun currentStaticRegionId(): String? = staticRegionId

        @JvmStatic
        fun isRegionPrepared(regionId: String?): Boolean {
            val desired = normalizeRegionId(regionId)
            return desired == normalizeRegionId(staticRegionId) ||
                desired == normalizeRegionId(preloadedRegionId)
        }

        private fun normalizeRegionId(regionId: String?): String? =
            regionId?.trim()?.takeIf { it.isNotEmpty() }

        private fun destroyHandle(handle: Long) {
            if (handle != 0L) {
                nativeBridge.nativeDestroy(handle)
            }
        }

        private fun parseRegionList(json: String?): List<RegionOption> {
            if (json.isNullOrBlank()) return emptyList()
            val array = JSONArray(json)
            val regions = ArrayList<RegionOption>(array.length())
            for (i in 0 until array.length()) {
                val item = array.getJSONObject(i)
                val id = item.optString("id").trim()
                if (id.isEmpty()) continue
                val name = item.optString("name_zh").trim().ifBlank { id }
                regions += RegionOption(id = id, nameZh = name)
            }
            return regions
        }

        private data class KeySpec(val label: String, val weight: Float = 1f, val isLetter: Boolean = false)

        private val ROW_1 = listOf(
            KeySpec("q", isLetter=true), KeySpec("w", isLetter=true), KeySpec("e", isLetter=true),
            KeySpec("r", isLetter=true), KeySpec("t", isLetter=true), KeySpec("y", isLetter=true),
            KeySpec("u", isLetter=true), KeySpec("i", isLetter=true), KeySpec("o", isLetter=true),
            KeySpec("p", isLetter=true))
        private val ROW_2 = listOf(
            KeySpec("a", isLetter=true), KeySpec("s", isLetter=true), KeySpec("d", isLetter=true),
            KeySpec("f", isLetter=true), KeySpec("g", isLetter=true), KeySpec("h", isLetter=true),
            KeySpec("j", isLetter=true), KeySpec("k", isLetter=true), KeySpec("l", isLetter=true))
        private val ROW_3_LETTERS = listOf(
            KeySpec("z", isLetter=true), KeySpec("x", isLetter=true), KeySpec("c", isLetter=true),
            KeySpec("v", isLetter=true), KeySpec("b", isLetter=true), KeySpec("n", isLetter=true),
            KeySpec("m", isLetter=true))

        // Symbol page: all Chinese full-width punctuation
        private val SYM_ROW_1 = listOf(
            KeySpec("1"), KeySpec("2"), KeySpec("3"), KeySpec("4"), KeySpec("5"),
            KeySpec("6"), KeySpec("7"), KeySpec("8"), KeySpec("9"), KeySpec("0"))
        private val SYM_ROW_2 = listOf(
            KeySpec("\u3010"), KeySpec("\u3011"), KeySpec("\u201C"), KeySpec("\u201D"),
            KeySpec("\u3008"), KeySpec("\u3009"), KeySpec("\u300A"), KeySpec("\u300B"),
            KeySpec("\uFF1A"), KeySpec("\uFF1B"))
        private val SYM_ROW_3 = listOf(
            KeySpec("\uFF0C"), KeySpec("\u3001"), KeySpec("\u3002"), KeySpec("\uFF1F"),
            KeySpec("\uFF01"), KeySpec("\u2026"), KeySpec("\u2014"), KeySpec("\uFF5E"),
            KeySpec("\u00B7"), KeySpec("\uFF0F"))

        private const val KEY_BG        = 0xFFF0F0F0.toInt()
        private const val KEY_BG_ACTION = 0xFFD0D8E0.toInt()
        private const val KEY_TEXT      = 0xFF222222.toInt()
        private const val CANDIDATE_BG  = 0xFFE8ECF0.toInt()

        init { System.loadLibrary("gannyu_input_jni") }
    }

    override fun onCreate() {
        super.onCreate()
        loadSelectedPipelineAsync()
    }

    override fun onDestroy() {
        // 不销毁 pipeline——staticHandle 保持全局唯一实例
        pipelineHandle = 0
        pipelineReady = false
        super.onDestroy()
    }

    override fun onCreateInputView(): View {
        val root = LayoutInflater.from(this).inflate(R.layout.input_view, null)
        preeditView = root.findViewById(R.id.preeditView)
        candidateScroll = root.findViewById(R.id.candidateScroll)
        candidateBar = root.findViewById(R.id.candidateBar)
        keyboardRows = root.findViewById(R.id.keyboardRows)
        renderKeyboard()
        renderState()
        return root
    }

    override fun onStartInput(attribute: android.view.inputmethod.EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        resetState(clearAccumulated = true)
    }

    override fun onStartInputView(info: android.view.inputmethod.EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        if (selectedRegionId(this) != currentStaticRegionId()) {
            loadSelectedPipelineAsync()
        }
        renderState()
    }

    override fun onFinishInput() {
        resetState(clearAccumulated = true)
        super.onFinishInput()
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        // Input view is being finished (e.g., switching to another IME) — clear UI and composing state
        resetState(clearAccumulated = true)
        super.onFinishInputView(finishingInput)
    }

    override fun onWindowHidden() {
        // Window hidden (IME no longer visible) — ensure we don't keep composing spans
        resetState(clearAccumulated = true)
        super.onWindowHidden()
    }

    override fun onEvaluateFullscreenMode(): Boolean = false

    private fun desiredRegionId(): String? = selectedRegionId(this)

    private fun loadSelectedPipelineAsync() {
        Thread {
            try {
                val desired = desiredRegionId()
                synchronized(preloadLock) {
                    if (staticHandle != 0L && staticRegionId == desired) {
                        pipelineHandle = staticHandle
                        pipelineReady = true
                        Log.i(TAG, "Pipeline reused for region=${desired ?: "(default)"}")
                        postUpdateCandidates()
                        return@Thread
                    }
                    if (staticHandle != 0L && staticRegionId != desired) {
                        destroyHandle(staticHandle)
                        staticHandle = 0L
                        staticRegionId = null
                    }
                    if (preloadedHandle != 0L && preloadedRegionId == desired) {
                        pipelineHandle = preloadedHandle
                        preloadedHandle = 0L
                        staticHandle = pipelineHandle
                        staticRegionId = desired
                        preloadedRegionId = null
                        pipelineReady = true
                        Log.i(TAG, "Pipeline reused from preload for region=${desired ?: "(default)"}")
                        postUpdateCandidates()
                        return@Thread
                    }
                    if (preloadedHandle != 0L && preloadedRegionId != desired) {
                        destroyHandle(preloadedHandle)
                        preloadedHandle = 0L
                        preloadedRegionId = null
                    }
                }
                // No handle available — delegate to preload so only one pipeline is created.
                Log.i(TAG, "Delegating to preload for region=${desired ?: "(default)"}")
                preloadSelectedRegionAsync(this, desired)
                synchronized(preloadLock) {
                    (preloadLock as Object).wait(20_000)
                    if (preloadedHandle != 0L) {
                        pipelineHandle = preloadedHandle
                        preloadedHandle = 0L
                        staticHandle = pipelineHandle
                        staticRegionId = desired
                        preloadedRegionId = null
                        pipelineReady = true
                        Log.i(TAG, "Pipeline loaded via preload for region=${desired ?: "(default)"}")
                        postUpdateCandidates()
                        return@Thread
                    }
                }
                Log.e(TAG, "Preload failed for region=${desired ?: "(default)"}")
                return@Thread
            } catch (e: Exception) {
                Log.e(TAG, "Pipeline init failed", e)
            }
        }.start()
    }

    // ===== Keyboard rendering =====

    private fun renderKeyboard() {
        keyboardRows.removeAllViews()
        val gap = dp(5)
        if (symbolPage) renderSymbolPage(gap) else renderPinyinPage(gap)
    }

    private fun renderPinyinPage(gap: Int) {
        keyboardRows.addView(keyRow(ROW_1, gap).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
        val r2 = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { bottomMargin = gap }
        }
        r2.addView(spacer(0.5f)); ROW_2.forEach { r2.addView(keyBtn(it, gap)) }; r2.addView(spacer(0.5f))
        keyboardRows.addView(r2)
        val r3 = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { bottomMargin = gap }
        }
        r3.addView(keyBtn(KeySpec("\u21E7", 1.3f), gap))
        ROW_3_LETTERS.forEach { r3.addView(keyBtn(it, gap)) }
        r3.addView(keyBtn(KeySpec("\u232B", 1.5f), gap))
        keyboardRows.addView(r3)
        val r4 = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT) }
        r4.addView(keyBtn(KeySpec("123", 1.2f), gap)); r4.addView(keyBtn(KeySpec("\uFF0C", 1f), gap))
        r4.addView(keyBtn(KeySpec("\u7A7A\u683C", 4.5f), gap))
        r4.addView(keyBtn(KeySpec("\u3002", 1f), gap)); r4.addView(keyBtn(KeySpec("\u21B5", 1.8f), gap))
        keyboardRows.addView(r4)
    }

    private fun renderSymbolPage(gap: Int) {
        keyboardRows.addView(keyRow(SYM_ROW_1, gap).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
        keyboardRows.addView(keyRow(SYM_ROW_2, gap).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
        keyboardRows.addView(keyRow(SYM_ROW_3, gap).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
        val r4 = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT) }
        r4.addView(keyBtn(KeySpec("\uFF08", 1f), gap)); r4.addView(keyBtn(KeySpec("\uFF09", 1f), gap))
        r4.addView(keyBtn(KeySpec("\u7A7A\u683C", 3f), gap))
        r4.addView(keyBtn(KeySpec("\u201C", 1f), gap)); r4.addView(keyBtn(KeySpec("\u21B5", 1.6f), gap))
        r4.addView(keyBtn(KeySpec("\u62FC", 1.2f), gap))
        keyboardRows.addView(r4)
    }

    private fun spacer(weight: Float): View = View(this).apply { layoutParams = LinearLayout.LayoutParams(0, 0, weight) }
    private fun keyRow(keys: List<KeySpec>, gap: Int): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL; layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        keys.forEach { addView(keyBtn(it, gap)) }
    }
    private fun keyBtn(key: KeySpec, gap: Int): Button = Button(this).apply {
        text = key.label; isAllCaps = false; textSize = 16f; setTextColor(KEY_TEXT)
        layoutParams = LinearLayout.LayoutParams(0, dp(42), key.weight).apply {
            if (key.label != "\uFF0C" && key.label != "\u3002") marginEnd = gap
        }
        setPadding(0, 0, 0, 0)
        setBackgroundColor(if (key.isLetter) KEY_BG else KEY_BG_ACTION)
        setOnClickListener { onKey(key) }
    }

    // ===== Key handling =====

    private fun onKey(key: KeySpec) {
        when {
            key.label == "\u232B"                        -> handleBackspace()
            key.label == "\u21B5"                        -> handleEnter()
            key.label == "\u7A7A\u683C"                  -> handleSpace()
            key.label == "123"                           -> { symbolPage = true; renderKeyboard() }
            key.label == "\u62FC"                        -> { symbolPage = false; renderKeyboard() }
            key.label == "\u201C"                       -> appendInput('“')
            key.label == "\u201D"                       -> appendInput('”')
            key.isLetter                                 -> appendInput(key.label.single())
            key.label in PUNCT_AFTER_COMPOSE             -> { maybeCommitComposing(); currentInputConnection?.commitText(key.label, 1) }
            else                                         -> currentInputConnection?.commitText(key.label, 1)
        }
    }

    private val PUNCT_AFTER_COMPOSE = setOf("\uFF0C", "\u3002", "\uFF1F", "\uFF01", "\uFF1A", "\uFF1B", "\u3001")
    private fun maybeCommitComposing() {
        if (composing.isNotEmpty()) {
            val first = lastCandidates.firstOrNull()
            if (first != null) commitCandidate(first) else commitRawBuffer()
        }
    }

    // ===== Input handling =====

    private fun appendInput(character: Char) { composing.append(character); renderState() }

    private fun handleBackspace() {
        if (composing.isNotEmpty()) {
            composing.deleteCharAt(composing.length - 1)
            if (composing.isEmpty()) {
                clearAccumulatedSelection()
                currentInputConnection?.setComposingText("", 1)
            } else {
                currentInputConnection?.setComposingText(composing.toString(), 1)
            }
            renderState()
            return
        }
        currentInputConnection?.deleteSurroundingText(1, 0)
    }

    private fun handleSpace() {
        if (composing.isEmpty()) { currentInputConnection?.commitText(" ", 1); return }
        val first = lastCandidates.firstOrNull()
        if (first != null) commitCandidate(first) else commitRawBuffer()
    }

    private fun handleEnter() {
        if (composing.isNotEmpty()) commitRawBuffer() else currentInputConnection?.commitText("\n", 1)
    }

    private fun commitRawBuffer() {
        if (composing.isEmpty()) return
        currentInputConnection?.commitText(composing.toString(), 1)
        resetState(clearAccumulated = true)
    }

    private fun commitCandidate(candidate: RankedCandidate) {
        val sanitized = candidate.text.replace('「', '“').replace('」', '”').replace('『','“').replace('』','”')
        currentInputConnection?.commitText(sanitized, 1)
        if (pipelineHandle != 0L) nativeUserDictBoost(pipelineHandle, sanitized)
        accumulatedText.append(sanitized)
        if (!candidate.reading.isNullOrBlank()) accumulatedReading += candidate.reading
        if (!candidate.mandarinReading.isNullOrBlank()) accumulatedMandarinReading += candidate.mandarinReading
        if (composing.length >= 4 && candidate.consumedBytes > 0 && candidate.consumedBytes < composing.length) {
            composing.delete(0, candidate.consumedBytes); renderState(); return
        }
        maybeSaveUserWord(); resetState(clearAccumulated = true)
    }

    private fun maybeSaveUserWord() {
        if (pipelineHandle == 0L) { clearAccumulatedSelection(); return }
        if (accumulatedText.codePointCount(0, accumulatedText.length) >= 2 && accumulatedReading.isNotEmpty()) {
            nativeUserDictAdd(pipelineHandle, accumulatedText.toString(), accumulatedReading.joinToString(" "), accumulatedMandarinReading.joinToString(" "))
        }
        clearAccumulatedSelection()
    }

    private fun clearAccumulatedSelection() { accumulatedText.clear(); accumulatedReading.clear(); accumulatedMandarinReading.clear() }

    private fun resetState(clearAccumulated: Boolean) {
        composing.clear(); lastCandidates = emptyList()
        if (clearAccumulated) clearAccumulatedSelection()
        currentInputConnection?.finishComposingText()
        if (::preeditView.isInitialized) renderState()
    }

    // ===== UI rendering =====

    private fun renderState() {
        if (!::preeditView.isInitialized) return
        if (::candidateBar.isInitialized) { refreshCandidates(); renderCandidateBar() }
        renderPreedit()
    }

    private fun renderPreedit() {
        if (!::preeditView.isInitialized) return
        if (composing.isEmpty()) {
            preeditView.text = getString(R.string.preedit_hint); preeditView.setTextColor(0xFF999999.toInt())
            currentInputConnection?.finishComposingText(); return
        }
        preeditView.setTextColor(0xFF222222.toInt())
        preeditView.text = segmentedBufferForDisplay(
            composing.toString(), lastCandidates.firstOrNull()?.consumedBytes ?: 0
        )
        currentInputConnection?.setComposingText(preeditView.text, 1)
    }

    private fun refreshCandidates() {
        if (!::candidateBar.isInitialized) return
        candidateBar.removeAllViews()
        if (!pipelineReady || pipelineHandle == 0L || composing.isEmpty()) {
            Log.w(TAG, "refreshCandidates SKIP ready=$pipelineReady handle=$pipelineHandle composing='$composing'")
            lastCandidates = emptyList(); return
        }
        val json = nativeRetrieve(pipelineHandle, composing.toString())
        Log.i(TAG, "nativeRetrieve input='$composing' result=${json?.length ?: 0} chars")
        lastCandidates = json?.let(::parseCandidates).orEmpty()
    }

    private fun postUpdateCandidates() {
        if (::candidateBar.isInitialized && composing.isNotEmpty()) candidateBar.post { renderState() }
    }

    private fun renderCandidateBar() {
        if (!::candidateBar.isInitialized || lastCandidates.isEmpty()) return
        candidateBar.removeAllViews()
        lastCandidates.forEachIndexed { index, c ->
            val cv = CandidateView(this, c, index)
            candidateBar.addView(cv)
        }
        candidateScroll.post { candidateScroll.scrollTo(0, 0) }
    }

    /** Candidate view aligned to Linux: main text + one metadata line. */
    private inner class CandidateView(
        context: android.content.Context,
        private val candidate: RankedCandidate,
        index: Int
    ) : LinearLayout(context) {
        private var downX = 0f
        private var moved = false

        init {
            orientation = VERTICAL
            setBackgroundColor(CANDIDATE_BG)
            setPadding(dp(10), dp(6), dp(10), dp(6))
            minimumWidth = dp(56)
            layoutParams = LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                if (index < lastCandidates.size - 1) marginEnd = dp(4)
            }

            addView(TextView(context).apply {
                text = candidate.text
                textSize = 18f; setTextColor(0xFF222222.toInt())
                gravity = android.view.Gravity.CENTER_HORIZONTAL
            })

            val meta = buildCandidateMeta(candidate)
            if (meta.isNotEmpty()) {
                addView(TextView(context).apply {
                    text = meta
                    textSize = 11f; setTextColor(0xFF888888.toInt())
                    gravity = android.view.Gravity.CENTER_HORIZONTAL
                    maxLines = 1; setSingleLine(true)
                })
            }
        }

        override fun onTouchEvent(event: MotionEvent): Boolean {
            when (event.action) {
                MotionEvent.ACTION_DOWN -> { downX = event.x; moved = false; parent.requestDisallowInterceptTouchEvent(true); return true }
                MotionEvent.ACTION_MOVE -> {
                    if (kotlin.math.abs(event.x - downX) > dp(8)) { moved = true; parent.requestDisallowInterceptTouchEvent(false) }
                    return true
                }
                MotionEvent.ACTION_UP -> { parent.requestDisallowInterceptTouchEvent(false); if (!moved) commitCandidate(candidate); return true }
                MotionEvent.ACTION_CANCEL -> { parent.requestDisallowInterceptTouchEvent(false); return true }
            }
            return super.onTouchEvent(event)
        }
    }

    private fun buildCandidateMeta(c: RankedCandidate): String {
        if (!c.comment.isNullOrBlank()) return c.comment
        return c.reading.orEmpty()
    }

    private fun parseCandidates(json: String): List<RankedCandidate> {
        val array = JSONArray(json); val list = ArrayList<RankedCandidate>(array.length())
        for (i in 0 until array.length()) {
            val item = array.getJSONObject(i)
            list += RankedCandidate(
                text = item.optString("text"), comment = candidateComment(item),
                consumedBytes = item.optInt("consumed_bytes", 0),
                reading = item.optString("reading").takeIf { it.isNotBlank() },
                mandarinReading = item.optString("mandarin_reading").takeIf { it.isNotBlank() },
                mandarinOnly = item.optBoolean("mandarin_only"))
        }
        return list
    }

    private fun candidateComment(item: JSONObject): String? {
        if (item.isNull("annotation")) return null
        val a = item.optString("annotation").takeIf { it.isNotBlank() }; if (a != null) return a
        return if (item.optBoolean("mandarin_only")) "\u005B\u5B98\u005D" else null
    }

    private fun segmentedBufferForDisplay(buffer: String, consumedBytes: Int): String {
        if (pipelineHandle == 0L) return buffer
        return nativeFormatPreedit(pipelineHandle, buffer, consumedBytes) ?: buffer
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()
}
