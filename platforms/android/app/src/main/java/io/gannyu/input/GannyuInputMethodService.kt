package io.gannyu.input

import android.content.Context
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.graphics.Typeface
import android.util.Log
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject

private class NativePipelineBridge {
    init { System.loadLibrary("gannyu_input_jni") }

    external fun nativeCreate(manifestPath: String?, regionId: String?, dataDir: String): Long
    external fun nativeLastError(): String?
    external fun nativeSnapshot(handle: Long): String?
    external fun nativeProcessKey(handle: Long, eventJson: String): String?
    external fun nativeSelectCandidate(handle: Long, globalIndex: Int): String?
    external fun nativeChangeCandidatePage(handle: Long, direction: Int): String?
    external fun nativeClearComposition(handle: Long): String?
    external fun nativeResetCurrentUserData(handle: Long): Boolean
    external fun nativeDestroy(handle: Long)
}

class GannyuInputMethodService : InputMethodService() {
    private enum class KeyboardPage { LETTERS, NUMBERS, SYMBOLS, SYMBOLS_MORE }

    private data class RankedCandidate(
        val text: String,
        val comment: String?,
        val reading: String?,
        val mandarinReading: String?,
        val globalIndex: Int,
        val pageIndex: Int,
        val deletable: Boolean = false
    )

    private data class EngineSnapshot(
        val handled: Boolean = false,
        val commitText: String? = null,
        val rawInput: String = "",
        val preedit: String = "",
        val caret: Int = 0,
        val candidates: List<RankedCandidate> = emptyList(),
        val highlightedIndex: Int? = null,
        val pageNumber: Int = 0,
        val hasPreviousPage: Boolean = false,
        val hasNextPage: Boolean = false,
        val schemaId: String? = null,
        val asciiMode: Boolean = false
    )

    data class RegionOption(val id: String, val nameZh: String) {
        val displayLabel: String
            get() = "$nameZh（$id）"
    }

    private var pipelineHandle: Long = 0
    private var pipelineReady: Boolean = false
    private lateinit var cacheTag: TextView
    private lateinit var candidateScroll: HorizontalScrollView
    private lateinit var candidateBar: LinearLayout
    private lateinit var candidateExpandButton: Button
    private lateinit var candidateExpansionContainer: FrameLayout
    private lateinit var candidateExpandedScroll: android.widget.ScrollView
    private lateinit var candidateExpandedRows: LinearLayout
    private lateinit var keyboardRows: LinearLayout
    private lateinit var keyPreview: TextView
    private var lastSnapshot = EngineSnapshot()
    private var candidateExpanded = false
    private val expandedCandidates = mutableListOf<RankedCandidate>()
    private var keyboardPage = KeyboardPage.LETTERS
    private var englishMode = false
    private var lastRenderedKeyboardWidth = 0
    // One-shot state only: it is never persisted and resets after one letter.
    private var englishShift = false
    private val backspaceRepeatHandler = Handler(Looper.getMainLooper())
    private val backspaceRepeat = object : Runnable {
        override fun run() {
            handleBackspace()
            backspaceRepeatHandler.postDelayed(this, BACKSPACE_REPEAT_INTERVAL_MS)
        }
    }

    external fun nativeCreate(manifestPath: String?, regionId: String?, dataDir: String): Long
    external fun nativeLastError(): String?
    external fun nativeSnapshot(handle: Long): String?
    external fun nativeProcessKey(handle: Long, eventJson: String): String?
    external fun nativeSelectCandidate(handle: Long, globalIndex: Int): String?
    external fun nativeChangeCandidatePage(handle: Long, direction: Int): String?
    external fun nativeClearComposition(handle: Long): String?
    external fun nativeResetCurrentUserData(handle: Long): Boolean
    external fun nativeDestroy(handle: Long)
    external fun nativeEntryCount(handle: Long): Int

    companion object {
        private const val TAG = "GonnyuIME"
        private const val PREFS_NAME = "gannyu.runtime"
        private const val KEY_SELECTED_REGION = "selected_region"
        const val USER_DATA_CURRENT_REGION = 1
        const val USER_DATA_ALL_REGIONS = 2
        @Volatile var preloadedHandle: Long = 0
        @JvmField val preloadLock = Object()
        @Volatile private var staticHandle: Long = 0
        @Volatile private var staticRegionId: String? = null
        @Volatile private var preloadedRegionId: String? = null
        private val nativeBridge = NativePipelineBridge()

        @JvmStatic
        fun nativeCreateStatic(context: Context, region: String?): Long {
            val resourceRoot = RimeResourceStore.prepare(context).absolutePath
            val userDataDir = RimeResourceStore.userDataDirectory(context).absolutePath
            return nativeBridge.nativeCreate(resourceRoot, region, userDataDir)
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
        fun availableRegions(context: Context): List<RegionOption> =
            parseRegionList(RimeResourceStore.regionList(context))

        @JvmStatic
        fun clearUserDataAsync(context: Context, target: Int, onComplete: (Boolean) -> Unit) {
            Thread {
                val regions = availableRegions(context)
                val selected = selectedRegionId(context) ?: regions.firstOrNull()?.id
                val targets = when (target) {
                    USER_DATA_CURRENT_REGION -> listOfNotNull(selected)
                    USER_DATA_ALL_REGIONS -> regions.map { it.id }
                    else -> emptyList()
                }.distinct()
                val success = targets.isNotEmpty() && synchronized(preloadLock) {
                    val liveHandles = linkedMapOf<String, LinkedHashSet<Long>>()
                    normalizeRegionId(staticRegionId)?.let { region ->
                        if (staticHandle != 0L) liveHandles.getOrPut(region, ::linkedSetOf) += staticHandle
                    }
                    normalizeRegionId(preloadedRegionId)?.let { region ->
                        if (preloadedHandle != 0L) {
                            liveHandles.getOrPut(region, ::linkedSetOf) += preloadedHandle
                        }
                    }
                    targets.all { region ->
                        val regionHandles = liveHandles[region].orEmpty()
                        if (regionHandles.isNotEmpty()) {
                            regionHandles.all(nativeBridge::nativeResetCurrentUserData)
                        } else {
                                val temporary = nativeCreateStatic(context, region)
                            if (temporary == 0L) {
                                false
                            } else {
                                try {
                                    nativeBridge.nativeResetCurrentUserData(temporary)
                                } finally {
                                    destroyHandle(temporary)
                                }
                            }
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
                    val handle = nativeCreateStatic(context, desiredRegion)
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

        private fun parseRegionList(array: JSONArray?): List<RegionOption> {
            if (array == null) return emptyList()
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

        private data class KeySpec(val label: String, val isLetter: Boolean = false)

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

        private val NUM_ROW_1 = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")
        private val NUM_ROW_2 = listOf("-", "/", ":", ";", "(", ")", "¥", "&", "@", "\"")
        private val NUM_ROW_3 = listOf(".", ",", "?", "!", "'", "%", "＋", "⌫")
        private val SYM_ROW_1 = listOf("【", "】", "“", "”", "〈", "〉", "《", "》", "：", "；")
        private val SYM_ROW_2 = listOf("，", "、", "。", "？", "！", "…", "—", "～", "·", "／")
        private val SYM_ROW_3 = listOf("更多", "（", "）", "[", "]", "{", "}", "#", "⌫")
        private val MORE_ROW_1 = listOf("+", "−", "=", "×", "÷", "<", ">", "^", "~", "_")
        private val MORE_ROW_2 = listOf("@", "#", "$", "¥", "€", "£", "&", "*", "\\", "|")
        private val MORE_ROW_3 = listOf("常用", "!", "?", "'", "\"", ":", ";", "／", "⌫")

        private const val FUNCTION_KEY_WIDTH_MULTIPLIER = 1.12f
        private const val SEGMENT_KEY_WIDTH_MULTIPLIER = 1.5f
        private const val IME_SWITCH_KEY = "🌐"
        private const val BACKSPACE_INITIAL_DELAY_MS = 380L
        private const val BACKSPACE_REPEAT_INTERVAL_MS = 55L

        init { System.loadLibrary("gannyu_input_jni") }
    }

    override fun onCreate() {
        super.onCreate()
        loadSelectedPipelineAsync()
    }

    private val keyTextColor: Int
        get() = getColor(R.color.keyboard_text)

    private val keySecondaryTextColor: Int
        get() = getColor(R.color.keyboard_secondary_text)

    override fun onDestroy() {
        stopBackspaceRepeat()
        // 不销毁 pipeline——staticHandle 保持全局唯一实例
        pipelineHandle = 0
        pipelineReady = false
        super.onDestroy()
    }

    override fun onCreateInputView(): View {
        val root = LayoutInflater.from(this).inflate(R.layout.input_view, null)
        candidateScroll = root.findViewById(R.id.candidateScroll)
        candidateBar = root.findViewById(R.id.candidateBar)
        candidateExpandButton = root.findViewById(R.id.candidateExpandButton)
        candidateExpansionContainer = root.findViewById(R.id.candidateExpansionContainer)
        candidateExpandedScroll = root.findViewById(R.id.candidateExpandedScroll)
        candidateExpandedRows = root.findViewById(R.id.candidateExpandedRows)
        candidateExpandButton.setOnClickListener { toggleCandidateExpansion() }
        val overlay = candidateExpansionContainer
        (overlay.parent as? ViewGroup)?.removeView(overlay)
        (root as? FrameLayout)?.addView(overlay, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(154)).apply {
            gravity = android.view.Gravity.TOP or android.view.Gravity.START
            leftMargin = dp(6)
            topMargin = dp(20)
            rightMargin = dp(6)
        })
        val collapseButton = Button(this).apply {
            text = "⌃"
            textSize = 17f
            setTextColor(keyTextColor)
            background = getDrawable(R.drawable.key_action)
            elevation = dp(4).toFloat()
            setOnClickListener { toggleCandidateExpansion() }
        }
        overlay.addView(collapseButton, FrameLayout.LayoutParams(dp(32), dp(32)).apply {
            gravity = android.view.Gravity.TOP or android.view.Gravity.END
            topMargin = dp(4)
            rightMargin = dp(4)
        })
        keyboardRows = root.findViewById(R.id.keyboardRows)
        keyPreview = root.findViewById(R.id.keyPreview)
        cacheTag = root.findViewById(R.id.cacheTag)
        keyboardRows.addOnLayoutChangeListener { _, left, _, right, _, _, _, _, _ ->
            val width = right - left
            if (width > 0 && width != lastRenderedKeyboardWidth) {
                keyboardRows.post { if (keyboardRows.width != lastRenderedKeyboardWidth) renderKeyboard() }
            }
        }
        renderKeyboard()
        renderState()
        return root
    }

    override fun onStartInput(attribute: android.view.inputmethod.EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        englishShift = false
        resetState(clearAccumulated = true)
    }

    override fun onStartInputView(info: android.view.inputmethod.EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        if (selectedRegionId(this) != currentStaticRegionId()) {
            loadSelectedPipelineAsync()
        }
        renderKeyboard()
        renderState()
    }

    override fun onFinishInput() {
        hideKeyPreview()
        englishShift = false
        resetState(clearAccumulated = true)
        super.onFinishInput()
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        // Input view is being finished (e.g., switching to another IME) — clear UI and composing state
        englishShift = false
        hideKeyPreview()
        resetState(clearAccumulated = true)
        super.onFinishInputView(finishingInput)
    }

    override fun onUpdateSelection(
        oldSelStart: Int,
        oldSelEnd: Int,
        newSelStart: Int,
        newSelEnd: Int,
        candidatesStart: Int,
        candidatesEnd: Int,
    ) {
        super.onUpdateSelection(oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd)
    }

    override fun onWindowHidden() {
        // Window hidden (IME no longer visible) — ensure we don't keep composing spans
        englishShift = false
        hideKeyPreview()
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
        hideKeyPreview()
        keyboardRows.removeAllViews()
        val width = keyboardRows.width.takeIf { it > 0 } ?: resources.displayMetrics.widthPixels - dp(12)
        lastRenderedKeyboardWidth = width
        val gap = dp(if (width <= dp(315)) 4 else if (width <= dp(350)) 5 else 6)
        val keyWidth = (width - gap * 9) / 10
        when (keyboardPage) {
            KeyboardPage.LETTERS -> renderPinyinPage(keyWidth, gap)
            KeyboardPage.NUMBERS -> renderAuxiliaryPage(NUM_ROW_1, NUM_ROW_2, NUM_ROW_3, keyWidth, gap)
            KeyboardPage.SYMBOLS -> renderAuxiliaryPage(SYM_ROW_1, SYM_ROW_2, SYM_ROW_3, keyWidth, gap)
            KeyboardPage.SYMBOLS_MORE -> renderAuxiliaryPage(MORE_ROW_1, MORE_ROW_2, MORE_ROW_3, keyWidth, gap)
        }
        renderBottomRow(keyWidth, gap)
    }

    private fun renderPinyinPage(keyWidth: Int, gap: Int) {
        keyboardRows.addView(keyRow(ROW_1, keyWidth, gap).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
        val r2 = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { bottomMargin = gap }
        }
        r2.addView(spacer((keyWidth + gap) / 2))
        ROW_2.forEachIndexed { index, key -> r2.addView(keyBtn(key, keyWidth, if (index == ROW_2.lastIndex) 0 else gap)) }
        r2.addView(spacer((keyWidth + gap) / 2))
        keyboardRows.addView(r2)
        val third = listOf(KeySpec(if (englishMode) "⇧" else "分词")) + ROW_3_LETTERS + KeySpec("⌫")
        keyboardRows.addView(keyRow(third, keyWidth, gap, deleteExtended = true).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
    }

    private fun renderAuxiliaryPage(one: List<String>, two: List<String>, three: List<String>, keyWidth: Int, gap: Int) {
        keyboardRows.addView(keyRow(one.map(::KeySpec), keyWidth, gap).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
        keyboardRows.addView(keyRow(two.map(::KeySpec), keyWidth, gap).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
        keyboardRows.addView(keyRow(three.map(::KeySpec), keyWidth, gap, deleteExtended = true).apply { (layoutParams as? LinearLayout.LayoutParams)?.bottomMargin = gap })
    }

    private fun renderBottomRow(keyWidth: Int, gap: Int) {
        val r4 = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT) }
        val showImeSwitcher = shouldShowImeSwitchKey()
        val mode = if (keyboardPage == KeyboardPage.LETTERS) if (englishMode) "中" else "英"
            else if (keyboardPage == KeyboardPage.NUMBERS) "符号" else "123"
        val nav = if (keyboardPage == KeyboardPage.LETTERS) "123" else "ABC"
        val keys = (if (showImeSwitcher) listOf(IME_SWITCH_KEY) else emptyList()) +
            listOf(mode, nav, "空格", if (englishMode) "," else "，", if (englishMode) "." else "。", "↵")
        keys.forEachIndexed { index, label ->
            val width = when (label) {
                "空格" -> 0
                "中", "英", "123", "ABC", "符号", IME_SWITCH_KEY -> functionKeyWidth(label, keyWidth)
                "↵" -> (keyWidth * 1.6f).toInt()
                else -> keyWidth
            }
            r4.addView(keyBtn(KeySpec(label), width, if (index == keys.lastIndex) 0 else gap))
        }
        keyboardRows.addView(r4)
    }

    private fun spacer(width: Int): View = View(this).apply { layoutParams = LinearLayout.LayoutParams(width, 1) }
    private fun keyRow(keys: List<KeySpec>, keyWidth: Int, gap: Int, deleteExtended: Boolean = false): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL; layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        val functionalExtra = if (deleteExtended) {
            keys.dropLast(1).fold(0) { total, key -> total + functionKeyExtraWidth(key.label, keyWidth) }
        } else {
            0
        }
        keys.forEachIndexed { index, key ->
            val width = if (deleteExtended && index == keys.lastIndex) {
                2 * keyWidth + gap - functionalExtra
            } else {
                functionKeyWidth(key.label, keyWidth)
            }
            addView(keyBtn(key, width, if (index == keys.lastIndex) 0 else gap))
        }
    }

    private fun functionKeyWidth(label: String, keyWidth: Int): Int =
        when {
            label == "分词" -> (keyWidth * SEGMENT_KEY_WIDTH_MULTIPLIER).toInt()
            label in FUNCTION_WIDTH_KEYS -> (keyWidth * FUNCTION_KEY_WIDTH_MULTIPLIER).toInt()
            else -> keyWidth
        }

    private fun functionKeyExtraWidth(label: String, keyWidth: Int): Int = functionKeyWidth(label, keyWidth) - keyWidth
    private fun keyBtn(key: KeySpec, width: Int, gap: Int): Button = Button(this).apply {
        text = when {
            key.label == IME_SWITCH_KEY -> ""
            key.label == "空格" -> ""
            key.label == "↵" -> if (englishMode) "return" else "换行"
            key.isLetter && englishMode && englishShift -> key.label.uppercase()
            else -> key.label
        }
        isAllCaps = false; textSize = if (key.isLetter) 23f else if (key.label.length > 2) 12f else 15f
        if (key.isLetter) setTypeface(Typeface.DEFAULT_BOLD)
        if (key.label == "空格") contentDescription = "空格"
        layoutParams = LinearLayout.LayoutParams(width, dp(46), if (width == 0) 1f else 0f).apply {
            marginEnd = gap
        }
        setPadding(0, 0, 0, 0)
        val useActionStyle = key.label in ACTION_KEYS
        setTextColor(keyTextColor)
        setBackgroundResource(if (useActionStyle) R.drawable.key_action else R.drawable.key_normal)
        if (key.label == IME_SWITCH_KEY) {
            val icon = getDrawable(R.drawable.ic_globe)?.mutate()
            icon?.setTint(keyTextColor)
            icon?.setBounds(0, 0, dp(18), dp(18))
            setCompoundDrawables(icon, null, null, null)
            gravity = android.view.Gravity.CENTER
            contentDescription = "切换输入法"
            setOnClickListener { switchToNextEnabledInputMethod() }
            setOnLongClickListener {
                showSystemInputMethodPicker()
                true
            }
        } else if (key.label == "\u232B") {
            setOnTouchListener { _, event ->
                when (event.actionMasked) {
                    MotionEvent.ACTION_DOWN -> {
                        handleBackspace()
                        backspaceRepeatHandler.postDelayed(backspaceRepeat, BACKSPACE_INITIAL_DELAY_MS)
                        true
                    }
                    MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                        stopBackspaceRepeat()
                        true
                    }
                    else -> true
                }
            }
        } else {
            if (supportsKeyPreview(key)) {
                setOnTouchListener { view, event ->
                    when (event.actionMasked) {
                        MotionEvent.ACTION_DOWN -> showKeyPreview(view, text.toString())
                        MotionEvent.ACTION_MOVE -> if (event.x !in 0f..view.width.toFloat() || event.y !in 0f..view.height.toFloat()) hideKeyPreview()
                        MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_OUTSIDE -> hideKeyPreview()
                    }
                    false
                }
            }
            setOnClickListener { onKey(key) }
        }
    }

    private fun supportsKeyPreview(key: KeySpec): Boolean =
        key.label.length == 1 && key.label !in ACTION_KEYS

    private fun showKeyPreview(anchor: View, label: String) {
        if (!::keyPreview.isInitialized) return
        val root = keyPreview.parent as? View ?: return
        val anchorLocation = IntArray(2)
        val rootLocation = IntArray(2)
        anchor.getLocationInWindow(anchorLocation)
        root.getLocationInWindow(rootLocation)
        val width = dp(58)
        val height = dp(66)
        val centerX = anchorLocation[0] - rootLocation[0] + anchor.width / 2
        val left = centerX.coerceIn(width / 2 + dp(3), root.width - width / 2 - dp(3)) - width / 2
        val top = (anchorLocation[1] - rootLocation[1] - height - dp(5)).coerceAtLeast(0)
        keyPreview.text = label
        keyPreview.layoutParams = (keyPreview.layoutParams as FrameLayout.LayoutParams).apply {
            leftMargin = left
            topMargin = top
        }
        keyPreview.visibility = View.VISIBLE
        keyPreview.bringToFront()
    }

    private fun hideKeyPreview() {
        if (::keyPreview.isInitialized) keyPreview.visibility = View.GONE
    }

    // ===== Key handling =====

    private fun onKey(key: KeySpec) {
        when {
            key.label == "\u232B"                        -> handleBackspace()
            key.label == "\u21B5"                        -> handleEnter()
            key.label == "\u7A7A\u683C"                  -> handleSpace()
            key.label == "\u82F1" || key.label == "\u4E2D" -> {
                resetState(clearAccumulated = true)
                englishMode = !englishMode
                englishShift = false
                renderKeyboard()
            }
            key.label == "⇧" && englishMode            -> { englishShift = !englishShift; renderKeyboard() }
            key.label == "分词"                            -> appendInput('\'')
            key.label == "123"                           -> { englishShift = false; keyboardPage = KeyboardPage.NUMBERS; renderKeyboard() }
            key.label == "符号"                            -> { keyboardPage = KeyboardPage.SYMBOLS; renderKeyboard() }
            key.label == "更多"                            -> { keyboardPage = KeyboardPage.SYMBOLS_MORE; renderKeyboard() }
            key.label == "常用"                            -> { keyboardPage = KeyboardPage.SYMBOLS; renderKeyboard() }
            key.label == "ABC"                           -> { keyboardPage = KeyboardPage.LETTERS; renderKeyboard() }
            key.isLetter                                 -> if (englishMode) {
                currentInputConnection?.commitText(if (englishShift) key.label.uppercase() else key.label, 1)
                if (englishShift) { englishShift = false; renderKeyboard() }
            } else appendInput(key.label.single())
            keyboardPage != KeyboardPage.LETTERS || key.label in PUNCT_AFTER_COMPOSE -> {
                if (lastSnapshot.rawInput.isNotEmpty()) applyEngineSnapshot(processEngineText(key.label))
                else currentInputConnection?.commitText(key.label, 1)
            }
            else                                         -> currentInputConnection?.commitText(key.label, 1)
        }
    }

    private val PUNCT_AFTER_COMPOSE = setOf("，", "。", "？", "！", "：", "；", "、", ",", ".")
    private val ACTION_KEYS = setOf("分词", "⇧", "⌫", "中", "英", "123", "ABC", "符号", "更多", "常用", "↵", IME_SWITCH_KEY)
    private val FUNCTION_WIDTH_KEYS = setOf("分词", "⇧", "中", "英", "123", "ABC", "符号", "更多", "常用", IME_SWITCH_KEY)

    private fun shouldShowImeSwitchKey(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            shouldOfferSwitchingToNextInputMethod()
        } else {
            val manager = getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
            manager?.enabledInputMethodList?.size?.let { it > 1 } == true
        }
    }

    private fun switchToNextEnabledInputMethod() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P && switchToNextInputMethod(false)) return
        showSystemInputMethodPicker()
    }

    private fun showSystemInputMethodPicker() {
        val manager = getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
        manager?.showInputMethodPicker()
    }

    // ===== Input handling =====

    private fun appendInput(character: Char) {
        applyEngineSnapshot(processEngineText(character.toString()))
    }

    private fun handleBackspace() {
        if (lastSnapshot.rawInput.isNotEmpty()) {
            applyEngineSnapshot(processEngineEvent("""{"type":"backspace"}"""))
            return
        }
        currentInputConnection?.deleteSurroundingTextInCodePoints(1, 0)
    }

    private fun stopBackspaceRepeat() {
        backspaceRepeatHandler.removeCallbacks(backspaceRepeat)
    }

    private fun handleSpace() {
        if (lastSnapshot.rawInput.isEmpty()) {
            currentInputConnection?.commitText(" ", 1)
            return
        }
        applyEngineSnapshot(processEngineEvent("""{"type":"space"}"""))
    }

    private fun handleEnter() {
        if (lastSnapshot.rawInput.isNotEmpty()) {
            applyEngineSnapshot(processEngineEnter())
        } else {
            currentInputConnection?.commitText("\n", 1)
        }
    }

    private fun processEngineEnter(): EngineSnapshot? = processEngineEvent("""{"type":"enter"}""")

    private fun commitCandidate(candidate: RankedCandidate) {
        if (pipelineHandle == 0L) return
        // The expanded panel represents one candidate snapshot.  Selecting an
        // item invalidates it, so hide it before requesting the next snapshot.
        if (candidateExpanded) collapseCandidateExpansion()
        applyEngineSnapshot(nativeSelectCandidate(pipelineHandle, candidate.globalIndex)?.let(::parseSnapshot))
    }

    private fun resetState(clearAccumulated: Boolean) {
        if (pipelineHandle != 0L && clearAccumulated) {
            applyEngineSnapshot(nativeClearComposition(pipelineHandle)?.let(::parseSnapshot), render = false)
        }
        lastSnapshot = EngineSnapshot()
        if (::candidateBar.isInitialized) renderState()
    }

    // ===== UI rendering =====

    private fun renderState() {
        if (!::candidateBar.isInitialized) return
        if (::candidateBar.isInitialized) refreshCandidates()
        renderCacheTag()
        if (::candidateBar.isInitialized) renderCandidateBar()
        if (::candidateExpandedRows.isInitialized) renderExpandedCandidates()
    }

    private fun renderCacheTag() {
        if (!::cacheTag.isInitialized) return
        val display = lastSnapshot.preedit
        cacheTag.visibility = if (display.isEmpty()) View.INVISIBLE else View.VISIBLE
        cacheTag.text = display
    }

    private fun refreshCandidates() {
        if (!::candidateBar.isInitialized) return
        candidateBar.removeAllViews()
        if (!pipelineReady || pipelineHandle == 0L) {
            Log.w(TAG, "refreshCandidates SKIP ready=$pipelineReady handle=$pipelineHandle")
            lastSnapshot = EngineSnapshot()
            return
        }
        lastSnapshot = nativeSnapshot(pipelineHandle)?.let(::parseSnapshot) ?: EngineSnapshot()
    }

    private fun postUpdateCandidates() {
        if (::candidateBar.isInitialized) candidateBar.post { renderState() }
    }

    private fun renderCandidateBar() {
        if (!::candidateBar.isInitialized) return
        candidateBar.removeAllViews()
        candidateBar.setPadding(0, 0, 0, 0)
        candidateExpandButton.visibility = if (lastSnapshot.candidates.isEmpty()) View.GONE else View.VISIBLE
        candidateExpandButton.text = if (candidateExpanded) "⌃" else "⌄"
        if (lastSnapshot.candidates.isEmpty()) return
        lastSnapshot.candidates.forEachIndexed { index, c ->
            val cv = CandidateView(this, c, index, expanded = false)
            candidateBar.addView(cv)
        }
        candidateScroll.post { candidateScroll.scrollTo(0, 0) }
    }

    private inner class CandidateView(
        context: android.content.Context,
        private val candidate: RankedCandidate,
        index: Int,
        expanded: Boolean,
    ) : LinearLayout(context) {
        private var downX = 0f
        private var moved = false

        init {
            orientation = VERTICAL
            setPadding(dp(5), dp(if (expanded) 2 else 1), dp(5), dp(if (expanded) 2 else 1))
            minimumWidth = dp(44)
            minimumHeight = dp(36)
            gravity = android.view.Gravity.START or android.view.Gravity.CENTER_VERTICAL
            layoutParams = LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                if (!expanded && index < lastSnapshot.candidates.size - 1) marginEnd = dp(3)
            }

            addView(TextView(context).apply {
                text = candidate.text
                textSize = 16f; setTextColor(keyTextColor)
                setTypeface(null, Typeface.NORMAL)
                maxLines = 1
                setSingleLine(true)
            })

            val meta = buildCandidateMeta(candidate)
            if (meta.isNotEmpty()) {
                addView(TextView(context).apply {
                    text = meta
                    textSize = 10f; setTextColor(keySecondaryTextColor)
                    maxLines = 1
                    setSingleLine(true)
                })
            }
            contentDescription = candidate.text + "，" + meta
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

    private fun renderExpandedCandidates() {
        if (!::candidateExpandedRows.isInitialized) return
        candidateExpandedRows.removeAllViews()
        if (!candidateExpanded) {
            candidateExpansionContainer.visibility = View.GONE
            return
        }
        candidateExpansionContainer.visibility = View.VISIBLE
        candidateExpansionContainer.layoutParams = candidateExpansionContainer.layoutParams.apply { height = dp(154) }
        val availableWidth = candidateExpandedScroll.width - dp(6)
        if (availableWidth <= 0) {
            candidateExpandedScroll.post { renderExpandedCandidates() }
            return
        }
        var row = expandedCandidateRow()
        var usedWidth = 0
        expandedCandidates.forEachIndexed { index, candidate ->
            val width = candidateWidth(candidate, availableWidth)
            if (usedWidth > 0 && usedWidth + dp(3) + width > availableWidth) {
                candidateExpandedRows.addView(row)
                row = expandedCandidateRow()
                usedWidth = 0
            }
            row.addView(CandidateView(this, candidate, index, expanded = true), LinearLayout.LayoutParams(width, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                if (usedWidth > 0) marginStart = dp(3)
            })
            usedWidth += (if (usedWidth == 0) 0 else dp(3)) + width
        }
        if (row.childCount > 0) candidateExpandedRows.addView(row)
    }

    private fun expandedCandidateRow(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = android.view.Gravity.TOP
    }

    private fun candidateWidth(candidate: RankedCandidate, maximum: Int): Int {
        val scale = resources.displayMetrics.scaledDensity
        val wordWidth = android.graphics.Paint().apply { textSize = 16f * scale }.measureText(candidate.text)
        val metaWidth = android.graphics.Paint().apply { textSize = 10f * scale }.measureText(buildCandidateMeta(candidate))
        return maxOf(dp(44), kotlin.math.ceil(maxOf(wordWidth, metaWidth).toDouble()).toInt() + dp(10))
    }

    private fun toggleCandidateExpansion() {
        if (candidateExpanded) {
            collapseCandidateExpansion()
            return
        }
        if (lastSnapshot.candidates.isEmpty()) return
        candidateExpanded = true
        expandedCandidates.clear()
        expandedCandidates += lastSnapshot.candidates
        renderState()
    }

    private fun collapseCandidateExpansion() {
        candidateExpanded = false
        expandedCandidates.clear()
        renderState()
    }

    private fun processEngineText(text: String): EngineSnapshot? {
        val escaped = JSONObject.quote(text)
        return processEngineEvent("""{"type":"text","text":$escaped}""")
    }

    private fun processEngineEvent(eventJson: String): EngineSnapshot? {
        if (pipelineHandle == 0L || !pipelineReady) return null
        return nativeProcessKey(pipelineHandle, eventJson)?.let(::parseSnapshot)
    }

    private fun applyEngineSnapshot(snapshot: EngineSnapshot?, render: Boolean = true) {
        if (snapshot == null) return
        val compositionChanged = lastSnapshot.rawInput != snapshot.rawInput || !snapshot.commitText.isNullOrEmpty()
        if (!snapshot.commitText.isNullOrEmpty()) {
            currentInputConnection?.commitText(snapshot.commitText, 1)
        }
        lastSnapshot = snapshot
        if (compositionChanged) {
            candidateExpanded = false
            expandedCandidates.clear()
        }
        if (render && ::candidateBar.isInitialized) {
            renderState()
        }
    }

    private fun parseSnapshot(json: String): EngineSnapshot {
        val item = JSONObject(json)
        val candidatesArray = item.optJSONArray("candidates") ?: JSONArray()
        val candidates = ArrayList<RankedCandidate>(candidatesArray.length())
        for (i in 0 until candidatesArray.length()) {
            val candidate = candidatesArray.getJSONObject(i)
            candidates += RankedCandidate(
                text = candidate.optString("text"),
                comment = candidate.optString("annotation").takeIf { it.isNotBlank() },
                reading = candidate.optString("reading").takeIf { it.isNotBlank() },
                mandarinReading = candidate.optString("mandarinReading").takeIf { it.isNotBlank() },
                globalIndex = candidate.optInt("globalIndex", i),
                pageIndex = candidate.optInt("pageIndex", i),
                deletable = candidate.optBoolean("deletable"),
            )
        }
        return EngineSnapshot(
            handled = item.optBoolean("handled"),
            commitText = item.optString("commitText").takeIf { it.isNotBlank() },
            rawInput = item.optString("rawInput"),
            preedit = item.optString("preedit"),
            caret = item.optInt("caret"),
            candidates = candidates,
            highlightedIndex = item.optInt("highlightedIndex").takeIf { it >= 0 },
            pageNumber = item.optInt("pageNumber"),
            hasPreviousPage = item.optBoolean("hasPreviousPage"),
            hasNextPage = item.optBoolean("hasNextPage"),
            schemaId = item.optString("schemaId").takeIf { it.isNotBlank() },
            asciiMode = item.optBoolean("asciiMode"),
        )
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()
}
