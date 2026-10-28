package io.gannyu.input

import android.content.Context
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.graphics.Typeface
import android.icu.text.BreakIterator
import android.text.InputType
import android.util.Log
import android.util.TypedValue
import android.view.KeyEvent
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject
import java.util.ArrayDeque
import java.util.concurrent.Executors
import java.util.Locale

private class NativePipelineBridge {
    init { System.loadLibrary("gannyu_input_jni") }

    external fun nativeCreate(manifestPath: String?, regionId: String?, dataDir: String): Long
    external fun nativeLastError(): String?
    external fun nativeSetInputOptions(handle: Long, values: IntArray): Boolean
    external fun nativeSnapshot(handle: Long): String?
    external fun nativeCandidates(handle: Long, offset: Int, limit: Int): String?
    external fun nativeProcessKey(handle: Long, eventJson: String): String?
    external fun nativeSelectCandidate(handle: Long, globalIndex: Int): String?
    external fun nativeChangeCandidatePage(handle: Long, direction: Int): String?
    external fun nativeClearComposition(handle: Long): String?
    external fun nativeResetCurrentUserData(handle: Long): Boolean
    external fun nativeDestroy(handle: Long)
}

class GannyuInputMethodService : InputMethodService() {
    private enum class KeyboardPage { LETTERS, NUMBERS, SYMBOLS, SYMBOLS_MORE, EMOJI }

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

    @Volatile private var pipelineHandle: Long = 0
    @Volatile private var pipelineReady: Boolean = false
    @Volatile private var pipelineLoadGeneration = 0L
    @Volatile private var engineGeneration = 0L
    private var hasPendingComposition = false
    private var pendingEngineOperationCount = 0
    private var needsEngineClear = true
    private val pendingEngineEvents = ArrayDeque<String>()
    private val engineExecutor = Executors.newSingleThreadExecutor()
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
    private var candidateLoadGeneration = 0
    private var candidateNextOffset = 0
    private var candidateHasMore = false
    private var candidateLoading = false
    private var candidateLoadFailed = false
    private val candidateLoadHandler = Handler(Looper.getMainLooper())
    private var keyboardPage = KeyboardPage.LETTERS
    private var editorKeyboardOverride = false
    private var savedKeyboardPage = KeyboardPage.LETTERS
    private var savedEnglishMode = false
    private var emojiPageIndex = 0
    private var englishMode = false
    private var lastRenderedKeyboardWidth = 0
    // One-shot state only: it is never persisted and resets after one letter.
    private var englishShift = false
    private var editorSelectionCollapsed: Boolean? = null
    private var editorSelectionStart: Int? = null
    private var editorSelectionEnd: Int? = null
    private var expectedSelectionStart: Int? = null
    private var expectedSelectionEnd: Int? = null
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
        @Volatile private var activeHandle: Long = 0
        @Volatile private var activeRegionId: String? = null
        private var preloadGeneration = 0L
        private val preloadCallbacks = mutableMapOf<String, MutableList<(Boolean, String?) -> Unit>>()
        private val preloadInFlight = mutableSetOf<String>()
        private val nativeBridge = NativePipelineBridge()

        @JvmStatic
        fun nativeCreateStatic(context: Context, region: String?): Long {
            val resourceRoot = RimeResourceStore.prepare(context).absolutePath
            val userDataDir = RimeResourceStore.userDataDirectory(context).absolutePath
            val handle = nativeBridge.nativeCreate(resourceRoot, region, userDataDir)
            if (handle != 0L && !nativeBridge.nativeSetInputOptions(handle, inputOptions(context))) {
                nativeBridge.nativeDestroy(handle)
                return 0
            }
            return handle
        }

        fun inputOptions(context: Context): IntArray {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            return IntArray(8) { index -> if (index == 5) prefs.getInt("input_option_5", 0).coerceIn(0, 2) else if (prefs.getBoolean("input_option_$index", true)) 1 else 0 }
        }

        fun populateInputOptions(context: Context, container: LinearLayout) {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            val values = inputOptions(context)
            val labels = listOf("副标题", "赣普词语连带", "模糊匹配", "普拼兼容", "其他模糊规则", "读音模式", "同义词连带", "文白匹配")
            val indices = listOf(0, 1, 6, 2, 3, 7, 4)
            val switches = indices.map { index ->
                val label = labels[index]
                android.widget.Switch(context).apply {
                    text = label
                    isChecked = values[index] != 0
                    val indent = if (index == 3 || index == 7 || index == 4) 36 else 12
                    setPadding((indent * resources.displayMetrics.density).toInt(), 8, 12, 8)
                    container.addView(this)
                }
            }
            fun updateEnabled() {
                switches[4].isEnabled = switches[3].isChecked
                switches[5].isEnabled = switches[3].isChecked
                switches[6].isEnabled = switches[3].isChecked
            }
            switches.forEachIndexed { index, control -> control.setOnCheckedChangeListener { _, checked ->
                prefs.edit().putBoolean("input_option_${indices[index]}", checked).apply()
                updateEnabled()
            } }
            updateEnabled()
            container.addView(TextView(context).apply { text = "读音模式"; setPadding(12, 12, 12, 0) })
            val modes = android.widget.RadioGroup(context)
            listOf("新老兼有", "新派", "老派").forEachIndexed { index, label ->
                modes.addView(android.widget.RadioButton(context).apply {
                    id = View.generateViewId()
                    text = label
                    isChecked = index == values[5]
                    tag = index
                })
            }
            modes.setOnCheckedChangeListener { group, id ->
                val selected = group.findViewById<View>(id)?.tag as? Int ?: return@setOnCheckedChangeListener
                prefs.edit().putInt("input_option_5", selected).apply()
            }
            container.addView(modes)
        }

        @JvmStatic
        fun nativeLastErrorStatic(): String? = nativeBridge.nativeLastError()

        @JvmStatic
        fun selectedRegionId(context: Context): String? {
            val saved = normalizeRegionId(
                context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                    .getString(KEY_SELECTED_REGION, null)
            )
            if (saved != null) return saved
            return runCatching { availableRegions(context).firstOrNull()?.id }.getOrNull()
        }

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
                    normalizeRegionId(activeRegionId)?.let { region ->
                        if (activeHandle != 0L) {
                            liveHandles.getOrPut(region, ::linkedSetOf) += activeHandle
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
            val key = desiredRegion ?: ""
            var launch = false
            var generation = 0L
            val immediate = synchronized(preloadLock) {
                val prepared = (staticHandle != 0L && desiredRegion == normalizeRegionId(staticRegionId)) ||
                    (preloadedHandle != 0L && desiredRegion == normalizeRegionId(preloadedRegionId)) ||
                    (activeHandle != 0L && desiredRegion == normalizeRegionId(activeRegionId))
                if (!prepared) {
                    if (onComplete != null) {
                        preloadCallbacks.getOrPut(key) { mutableListOf() } += onComplete
                    }
                    if (key !in preloadInFlight) {
                        preloadInFlight += key
                        generation = ++preloadGeneration
                        launch = true
                    }
                }
                prepared
            }
            if (immediate) {
                onComplete?.invoke(true, null)
                return
            }
            if (!launch) return

            Thread {
                var handle = 0L
                var success = false
                var detail: String? = null
                try {
                    handle = nativeCreateStatic(context, desiredRegion)
                    if (handle != 0L) {
                        success = true
                    } else {
                        detail = nativeLastErrorStatic()
                    }
                    Log.i(TAG, "preload region=${desiredRegion ?: "(default)"} handle=$handle")
                } catch (e: Exception) {
                    Log.e(TAG, "preload failed for region=${desiredRegion ?: "(default)"}", e)
                    detail = e.stackTraceToString()
                }

                lateinit var callbacks: List<(Boolean, String?) -> Unit>
                synchronized(preloadLock) {
                    preloadInFlight.remove(key)
                    callbacks = preloadCallbacks.remove(key).orEmpty()
                    if (success && handle != 0L) {
                        if (generation == preloadGeneration) {
                            val previous = preloadedHandle
                            preloadedHandle = handle
                            preloadedRegionId = desiredRegion
                            handle = 0L
                            if (previous != 0L) destroyHandle(previous)
                        } else {
                            success = false
                            detail = detail ?: "preload superseded"
                        }
                    }
                    preloadLock.notifyAll()
                }
                if (handle != 0L) destroyHandle(handle)
                val message = when {
                    success -> ""
                    !detail.isNullOrBlank() -> detail.orEmpty()
                    else -> "native pipeline creation returned no detail"
                }
                callbacks.forEach { callback -> runCatching { callback(success, message) } }
            }.start()
        }

        @JvmStatic
        fun currentStaticRegionId(): String? = staticRegionId.takeIf { staticHandle != 0L }

        @JvmStatic
        fun isRegionPrepared(regionId: String?): Boolean {
            val desired = normalizeRegionId(regionId)
            return (staticHandle != 0L && desired == normalizeRegionId(staticRegionId)) ||
                (preloadedHandle != 0L && desired == normalizeRegionId(preloadedRegionId)) ||
                (activeHandle != 0L && desired == normalizeRegionId(activeRegionId))
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
        private val NUM_ROW_2 = listOf("－", "／", "：", "；", "（", "）", "￥", "@", "“", "”")
        private val NUM_ROW_3 = listOf(".", ",", "？", "！", "＇", "％", "＋", "⌫")
        private val SYM_ROW_1 = listOf("【", "】", "“", "”", "〈", "〉", "《", "》", "：", "；")
        private val SYM_ROW_2 = listOf("，", "、", "。", "？", "！", "…", "—", "～", "·", "／")
        private val SYM_ROW_3 = listOf("更多", "（", "）", "[", "]", "{", "}", "#", "⌫")
        private val MORE_ROW_1 = listOf("+", "−", "=", "×", "÷", "<", ">", "^", "~", "_")
        private val MORE_ROW_2 = listOf("＠", "#", "$", "¥", "€", "£", "&", "*", "\\", "|")
        private val MORE_ROW_3 = listOf("常用", "!", "?", "'", "\"", ":", ";", "／", "⌫")

        private const val FUNCTION_KEY_WIDTH_MULTIPLIER = 1.12f
        private const val SEGMENT_KEY_WIDTH_MULTIPLIER = 1.5f
        private val EMOJI_PAGES = listOf(
            "😀 😃 😄 😁 😆 😅 😂 😊 😇 🙂 🙃 😉 😌 😍 😘 😗 😙 😚 😋 😛 😝 😜 🤗 🤔 😎 😏 😒 🙄 😬 😴".split(" "),
            "😔 😞 😟 😕 🙁 😣 😖 😫 😩 😢 😭 😤 😠 😡 😳 😱 😨 😰 😥 😓 😶 😐 😑 😯 😦 😧 😮 😲 😵 😷".split(" "),
            "👍 👎 👏 🙌 👐 🙏 👋 👌 👊 ✊ 💪 👈 👉 👆 👇 👀 👂 👃 👄 👶 👧 👦 👩 👨 👵 👴 👮 👷 👸 👼".split(" "),
            "🐶 🐱 🐭 🐹 🐰 🐻 🐼 🐨 🐯 🦁 🐮 🐷 🐸 🐵 🐔 🐧 🐦 🐤 🐺 🐗 🐴 🦄 🐝 🐛 🐌 🐞 🐜 🐢 🐍 🐙".split(" "),
            "🍎 🍐 🍊 🍋 🍌 🍉 🍇 🍓 🍒 🍑 🍍 🥝 🍅 🍆 🌽 🍠 🍞 🧀 🍖 🍗 🍔 🍟 🍕 🍳 🍜 🍚 🍣 🍦 🍰 🍺".split(" "),
            "🚗 🚕 🚌 🚎 🚓 🚑 🚒 🚚 🚲 🚂 🚆 🚇 🚊 🚉 🚀 🚁 🚢 🏠 🏡 🏢 🏥 🏫 🏪 🏨 🏭 🏰 🗼 🗽 🌋 🗻".split(" "),
            "🎉 🎊 🎁 🎈 🎂 🎄 🎃 🎆 🎇 💐 🌸 🌹 🌻 🌷 🌱 🌲 🌳 🌴 🌵 🍀 🍁 🍂 🍃 🌈 🌙 🌞 🌍 🌎 🌏 🔥".split(" "),
            "💖 💕 💞 💓 💗 💘 💝 💔 💜 💛 💚 💙 💯 💢 💥 💫 💦 💤 💬 💭 🔔 🎵 🎶 🎤 🎧 📷 📱 💻 🔑 💡".split(" "),
        )
        private const val IME_SWITCH_KEY = "🌐"
        private const val BACKSPACE_INITIAL_DELAY_MS = 380L
        private const val BACKSPACE_REPEAT_INTERVAL_MS = 55L

        init { System.loadLibrary("gannyu_input_jni") }
    }

    private val inputOptionsListener = android.content.SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
        if (key?.startsWith("input_option_") == true) {
            candidateLoadGeneration += 1
            candidateExpanded = false
            enqueueNativeOperation { handle ->
                check(nativeBridge.nativeSetInputOptions(handle, inputOptions(this)))
                nativeBridge.nativeSnapshot(handle)
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE).registerOnSharedPreferenceChangeListener(inputOptionsListener)
        loadSelectedPipelineAsync()
    }

    private val keyTextColor: Int
        get() = getColor(R.color.keyboard_text)

    private val keySecondaryTextColor: Int
        get() = getColor(R.color.keyboard_secondary_text)

    override fun onDestroy() {
        getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE).unregisterOnSharedPreferenceChangeListener(inputOptionsListener)
        stopBackspaceRepeat()
        pipelineLoadGeneration += 1
        engineGeneration += 1
        synchronized(preloadLock) { preloadLock.notifyAll() }
        candidateLoadGeneration += 1
        engineExecutor.shutdownNow()
        pipelineHandle = 0
        pipelineReady = false
        activeHandle = 0L
        activeRegionId = null
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
            setTextSize(TypedValue.COMPLEX_UNIT_DIP, 17f)
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
            val width = right - left - keyboardRows.paddingLeft - keyboardRows.paddingRight
            if (width > 0 && width != lastRenderedKeyboardWidth) {
                keyboardRows.post {
                    val currentWidth = keyboardRows.width - keyboardRows.paddingLeft - keyboardRows.paddingRight
                    if (currentWidth != lastRenderedKeyboardWidth) renderKeyboard()
                }
            }
        }
        root.addOnLayoutChangeListener { _, _, top, _, bottom, _, oldTop, _, oldBottom ->
            if (candidateExpanded && bottom - top != oldBottom - oldTop) renderExpandedCandidates()
        }
        renderKeyboard()
        renderState()
        return root
    }

    override fun onStartInput(attribute: android.view.inputmethod.EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        restoreEditorKeyboardState()
        applyEditorKeyboardState(attribute)
        englishShift = false
        editorSelectionCollapsed = attribute?.let {
            if (it.initialSelStart >= 0 && it.initialSelEnd >= 0) it.initialSelStart == it.initialSelEnd else null
        }
        editorSelectionStart = attribute?.initialSelStart?.takeIf { it >= 0 }
        editorSelectionEnd = attribute?.initialSelEnd?.takeIf { it >= 0 }
        expectedSelectionStart = null
        expectedSelectionEnd = null
        resetState(clearAccumulated = true)
    }

    override fun onStartInputView(info: android.view.inputmethod.EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        if (selectedRegionId(this) != currentStaticRegionId()) {
            loadSelectedPipelineAsync()
        }
        enqueueNativeOperation { handle ->
            check(nativeBridge.nativeSetInputOptions(handle, inputOptions(this)))
            nativeBridge.nativeSnapshot(handle)
        }
        renderKeyboard()
        renderState()
    }

    override fun onFinishInput() {
        stopBackspaceRepeat()
        hideKeyPreview()
        englishShift = false
        editorSelectionCollapsed = null
        editorSelectionStart = null
        editorSelectionEnd = null
        expectedSelectionStart = null
        expectedSelectionEnd = null
        restoreEditorKeyboardState()
        resetState(clearAccumulated = true)
        super.onFinishInput()
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        stopBackspaceRepeat()
        englishShift = false
        hideKeyPreview()
        restoreEditorKeyboardState()
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
        editorSelectionCollapsed = if (newSelStart >= 0 && newSelEnd >= 0) newSelStart == newSelEnd else null
        val expected = newSelStart >= 0 && newSelEnd >= 0 &&
            newSelStart == expectedSelectionStart && newSelEnd == expectedSelectionEnd
        editorSelectionStart = newSelStart.takeIf { it >= 0 }
        editorSelectionEnd = newSelEnd.takeIf { it >= 0 }
        if (expected) {
            expectedSelectionStart = null
            expectedSelectionEnd = null
            return
        }
        expectedSelectionStart = null
        expectedSelectionEnd = null
        if (hasPendingComposition &&
            (oldSelStart != newSelStart || oldSelEnd != newSelEnd) &&
            newSelStart >= 0 && newSelEnd >= 0
        ) {
            resetState(clearAccumulated = true)
        }
    }

    override fun onWindowHidden() {
        stopBackspaceRepeat()
        englishShift = false
        hideKeyPreview()
        resetState(clearAccumulated = true)
        super.onWindowHidden()
    }

    override fun onEvaluateFullscreenMode(): Boolean = false

    private fun desiredRegionId(): String? = selectedRegionId(this)

    private fun restoreEditorKeyboardState() {
        if (!editorKeyboardOverride) return
        keyboardPage = savedKeyboardPage
        englishMode = savedEnglishMode
        editorKeyboardOverride = false
    }

    private fun applyEditorKeyboardState(info: EditorInfo?) {
        val inputType = info?.inputType ?: return
        val inputClass = inputType and InputType.TYPE_MASK_CLASS
        val variation = inputType and InputType.TYPE_MASK_VARIATION
        val numeric = inputClass == InputType.TYPE_CLASS_NUMBER || inputClass == InputType.TYPE_CLASS_PHONE
        val asciiText = inputClass == InputType.TYPE_CLASS_TEXT && variation in setOf(
            InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
            InputType.TYPE_TEXT_VARIATION_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_URI,
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        )
        if (!numeric && !asciiText) return
        savedKeyboardPage = keyboardPage
        savedEnglishMode = englishMode
        editorKeyboardOverride = true
        keyboardPage = if (numeric) KeyboardPage.NUMBERS else KeyboardPage.LETTERS
        if (asciiText) englishMode = true
    }

    private fun loadSelectedPipelineAsync() {
        val generation = ++pipelineLoadGeneration
        engineGeneration += 1
        needsEngineClear = true
        if (pipelineReady && pipelineHandle != 0L) {
            pendingEngineEvents.clear()
            hasPendingComposition = false
            pendingEngineOperationCount = 0
            lastSnapshot = EngineSnapshot()
        }
        pipelineHandle = 0
        pipelineReady = false
        activeHandle = 0L
        activeRegionId = null
        synchronized(preloadLock) { preloadLock.notifyAll() }
        Thread {
            try {
                val desired = desiredRegionId()
                synchronized(preloadLock) {
                    if (generation != pipelineLoadGeneration || desired != desiredRegionId()) return@Thread
                    if (staticHandle != 0L && staticRegionId == desired) {
                        pipelineHandle = staticHandle
                        pipelineReady = true
                        activeHandle = pipelineHandle
                        activeRegionId = desired
                        Log.i(TAG, "Pipeline reused for region=${desired ?: "(default)"}")
                        postUpdateCandidates()
                        return@Thread
                    }
                    if (staticHandle != 0L && staticRegionId != desired) {
                        val previous = staticHandle
                        staticHandle = 0L
                        staticRegionId = null
                        destroyHandle(previous)
                    }
                    if (preloadedHandle != 0L && preloadedRegionId == desired) {
                        pipelineHandle = preloadedHandle
                        preloadedHandle = 0L
                        staticHandle = pipelineHandle
                        staticRegionId = desired
                        preloadedRegionId = null
                        pipelineReady = true
                        activeHandle = pipelineHandle
                        activeRegionId = desired
                        Log.i(TAG, "Pipeline reused from preload for region=${desired ?: "(default)"}")
                        postUpdateCandidates()
                        return@Thread
                    }
                    if (preloadedHandle != 0L && preloadedRegionId != desired) {
                        val previous = preloadedHandle
                        preloadedHandle = 0L
                        preloadedRegionId = null
                        destroyHandle(previous)
                    }
                }
                Log.i(TAG, "Delegating to preload for region=${desired ?: "(default)"}")
                preloadSelectedRegionAsync(this, desired)
                synchronized(preloadLock) {
                    val deadline = android.os.SystemClock.uptimeMillis() + 20_000
                    while (generation == pipelineLoadGeneration && desired == desiredRegionId() &&
                        (preloadedHandle == 0L || preloadedRegionId != desired)
                    ) {
                        val remaining = deadline - android.os.SystemClock.uptimeMillis()
                        if (remaining <= 0) break
                        (preloadLock as Object).wait(remaining)
                    }
                    if (generation != pipelineLoadGeneration || desired != desiredRegionId()) return@Thread
                    if (preloadedHandle != 0L && preloadedRegionId == desired) {
                        pipelineHandle = preloadedHandle
                        preloadedHandle = 0L
                        preloadedRegionId = null
                        staticHandle = pipelineHandle
                        staticRegionId = desired
                        pipelineReady = true
                        activeHandle = pipelineHandle
                        activeRegionId = desired
                        Log.i(TAG, "Pipeline loaded via preload for region=${desired ?: "(default)"}")
                        postUpdateCandidates()
                        return@Thread
                    }
                }
                Log.e(TAG, "Preload failed for region=${desired ?: "(default)"}")
                return@Thread
            } catch (e: Exception) {
                Log.e(TAG, "Pipeline init failed", e)
                if (generation == pipelineLoadGeneration) {
                    pipelineHandle = 0
                    pipelineReady = false
                    activeHandle = 0L
                    activeRegionId = null
                }
            }
        }.start()
    }

    // ===== Keyboard rendering =====

    private fun renderKeyboard() {
        hideKeyPreview()
        keyboardRows.removeAllViews()
        val measuredWidth = keyboardRows.width - keyboardRows.paddingLeft - keyboardRows.paddingRight
        val width = measuredWidth.takeIf { it > 0 } ?: resources.displayMetrics.widthPixels - dp(12)
        lastRenderedKeyboardWidth = width
        val gap = dp(if (width <= dp(315)) 4 else if (width <= dp(350)) 5 else 6)
        val keyWidth = (width - gap * 9) / 10
        when (keyboardPage) {
            KeyboardPage.LETTERS -> renderPinyinPage(keyWidth, gap)
            KeyboardPage.NUMBERS -> renderAuxiliaryPage(NUM_ROW_1, NUM_ROW_2, NUM_ROW_3, keyWidth, gap)
            KeyboardPage.SYMBOLS -> renderAuxiliaryPage(SYM_ROW_1, SYM_ROW_2, SYM_ROW_3, keyWidth, gap)
            KeyboardPage.SYMBOLS_MORE -> renderAuxiliaryPage(MORE_ROW_1, MORE_ROW_2, MORE_ROW_3, keyWidth, gap)
            KeyboardPage.EMOJI -> EMOJI_PAGES[emojiPageIndex].chunked(10).forEach { row ->
                keyboardRows.addView(keyRow(row.map(::KeySpec), keyWidth, gap, bottomGap = gap))
            }
        }
        renderBottomRow(keyWidth, gap)
    }

    private fun renderPinyinPage(keyWidth: Int, gap: Int) {
        keyboardRows.addView(keyRow(ROW_1, keyWidth, gap, bottomGap = gap))
        val r2 = KeyboardRowLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(46) + gap,
            )
        }
        r2.addView(spacer((keyWidth + gap) / 2))
        ROW_2.forEachIndexed { index, key -> r2.addView(keyBtn(key, keyWidth, if (index == ROW_2.lastIndex) 0 else gap)) }
        r2.addView(spacer((keyWidth + gap) / 2))
        keyboardRows.addView(r2)
        val third = listOf(KeySpec(if (englishMode) "⇧" else "分词")) + ROW_3_LETTERS + KeySpec("⌫")
        keyboardRows.addView(keyRow(third, keyWidth, gap, deleteExtended = true, bottomGap = gap))
    }

    private fun renderAuxiliaryPage(one: List<String>, two: List<String>, three: List<String>, keyWidth: Int, gap: Int) {
        keyboardRows.addView(keyRow(one.map(::KeySpec), keyWidth, gap, bottomGap = gap))
        keyboardRows.addView(keyRow(two.map(::KeySpec), keyWidth, gap, bottomGap = gap))
        keyboardRows.addView(keyRow(three.map(::KeySpec), keyWidth, gap, deleteExtended = true, bottomGap = gap))
    }

    private fun renderBottomRow(keyWidth: Int, gap: Int) {
        val r4 = KeyboardRowLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(46))
        }
        val keys = bottomRowLabels(shouldShowImeSwitchKey())
        keys.forEachIndexed { index, label ->
            val width = when (label) {
                "空格" -> 0
                "中", "英", "123", "ABC", "符号", IME_SWITCH_KEY -> functionKeyWidth(label, keyWidth)
                "↵" -> (keyWidth * 1.6f).toInt()
                "赣拼" -> (keyWidth * 1.5f).toInt()
                else -> keyWidth
            }
            r4.addView(keyBtn(KeySpec(label), width, if (index == keys.lastIndex) 0 else gap))
        }
        keyboardRows.addView(r4)
    }

    private fun bottomRowLabels(showImeSwitcher: Boolean): List<String> {
        val mode = if (keyboardPage == KeyboardPage.LETTERS) if (englishMode) "中" else "英"
            else if (keyboardPage == KeyboardPage.NUMBERS) "符号" else "123"
        val nav = if (keyboardPage == KeyboardPage.LETTERS) "123" else "ABC"
        val pageKeys = if (keyboardPage == KeyboardPage.EMOJI) {
            listOf("赣拼", "上一页", "下一页", "空格", "⌫", "↵")
        } else {
            listOf(mode, nav, "表情", "空格", if (englishMode) "," else "，", if (englishMode) "." else "。", "↵")
        }
        return (if (showImeSwitcher) listOf(IME_SWITCH_KEY) else emptyList()) + pageKeys
    }

    private fun spacer(width: Int): View = View(this).apply {
        layoutParams = LinearLayout.LayoutParams(width, 1)
    }

    private fun keyRow(
        keys: List<KeySpec>,
        keyWidth: Int,
        gap: Int,
        deleteExtended: Boolean = false,
        bottomGap: Int = 0,
    ): KeyboardRowLayout = KeyboardRowLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        layoutParams = LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            dp(46) + bottomGap,
        )
        keys.forEachIndexed { index, key ->
            val width = if (deleteExtended && index == keys.lastIndex) {
                deleteKeyWidth(keys, keyWidth, gap)
            } else {
                functionKeyWidth(key.label, keyWidth)
            }
            addView(keyBtn(key, width, if (index == keys.lastIndex) 0 else gap))
        }
    }

    private fun deleteKeyWidth(keys: List<KeySpec>, keyWidth: Int, gap: Int): Int {
        val occupiedWidth = keys.dropLast(1).sumOf { functionKeyWidth(it.label, keyWidth) + gap }
        return (keyWidth * 10 + gap * 9 - occupiedWidth).coerceAtLeast(keyWidth)
    }

    private fun functionKeyWidth(label: String, keyWidth: Int): Int =
        when {
            label == "分词" || label == "⇧" -> (keyWidth * SEGMENT_KEY_WIDTH_MULTIPLIER).toInt()
            label in FUNCTION_WIDTH_KEYS -> (keyWidth * FUNCTION_KEY_WIDTH_MULTIPLIER).toInt()
            else -> keyWidth
        }

    private fun keyBtn(key: KeySpec, width: Int, gap: Int): Button = Button(this).apply {
        text = when {
            key.label == IME_SWITCH_KEY -> ""
            key.label == "空格" -> ""
            key.label == "↵" -> if (englishMode) "return" else "回车"
            key.label == "表情" -> "☺"
            key.label == "上一页" -> "◀"
            key.label == "下一页" -> "▶"
            key.isLetter && englishMode && englishShift -> key.label.uppercase()
            else -> key.label
        }
        isAllCaps = false
        setTextSize(TypedValue.COMPLEX_UNIT_DIP, if (key.isLetter) 23f else if (key.label == "表情") 18f else if (key.label.length > 2) 12f else 15f)
        if (key.isLetter) setTypeface(Typeface.DEFAULT_BOLD)
        if (key.label == "空格") contentDescription = "空格"
        if (key.label == "↵") contentDescription = text
        if (key.label == "表情") contentDescription = "表情"
        if (key.label in setOf("上一页", "下一页")) contentDescription = "${key.label}，第${emojiPageIndex + 1}页，共${EMOJI_PAGES.size}页"
        if (keyboardPage == KeyboardPage.EMOJI && key.label !in ACTION_KEYS && key.label != "空格") {
            setTextSize(TypedValue.COMPLEX_UNIT_DIP, 23f)
        }
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
            foreground = icon
            foregroundGravity = android.view.Gravity.CENTER
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
            key.label == "表情" -> {
                resetState(clearAccumulated = true)
                emojiPageIndex = 0
                keyboardPage = KeyboardPage.EMOJI
                renderKeyboard()
            }
            key.label == "赣拼" -> {
                englishMode = false
                englishShift = false
                keyboardPage = KeyboardPage.LETTERS
                renderKeyboard()
            }
            key.label == "上一页" || key.label == "下一页" -> {
                val direction = if (key.label == "上一页") -1 else 1
                emojiPageIndex = (emojiPageIndex + direction + EMOJI_PAGES.size) % EMOJI_PAGES.size
                renderKeyboard()
            }
            key.label == "“" || key.label == "”"       -> {
                if (englishMode) {
                    currentInputConnection?.commitText("\"", 1)
                } else if (hasPendingComposition) {
                    applyEngineSnapshot(processEngineText(key.label))
                } else {
                    currentInputConnection?.commitText(key.label, 1)
                }
            }
            key.isLetter                                 -> if (englishMode) {
                currentInputConnection?.commitText(if (englishShift) key.label.uppercase() else key.label, 1)
                if (englishShift) { englishShift = false; renderKeyboard() }
            } else appendInput(key.label.single())
            keyboardPage != KeyboardPage.LETTERS || key.label in PUNCT_AFTER_COMPOSE -> {
                if (hasPendingComposition) applyEngineSnapshot(processEngineText(key.label))
                else currentInputConnection?.commitText(key.label, 1)
            }
            else                                         -> currentInputConnection?.commitText(key.label, 1)
        }
    }

    private val PUNCT_AFTER_COMPOSE = setOf("，", "。", "？", "！", "：", "；", "、", ",", ".")
    private val ACTION_KEYS = setOf("分词", "⇧", "⌫", "中", "英", "123", "ABC", "符号", "更多", "常用", "↵", "表情", "赣拼", "上一页", "下一页", IME_SWITCH_KEY)
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
        if (hasPendingComposition) {
            applyEngineSnapshot(processEngineEvent("""{"type":"backspace"}"""))
            return
        }
        val connection = currentInputConnection ?: return
        val selectedText = connection.getSelectedText(0)
        if (!selectedText.isNullOrEmpty()) {
            if (connection.commitText("", 1)) editorSelectionCollapsed = true
            return
        }
        if (selectedText == null && editorSelectionCollapsed != true) {
            sendDownUpKeyEvents(KeyEvent.KEYCODE_DEL)
            return
        }
        val beforeCursor = connection.getTextBeforeCursor(32, 0)?.toString()
        if (beforeCursor.isNullOrEmpty()) {
            sendDownUpKeyEvents(KeyEvent.KEYCODE_DEL)
            return
        }
        val iterator = BreakIterator.getCharacterInstance(Locale.ROOT)
        iterator.setText(beforeCursor)
        val clusterStart = iterator.preceding(beforeCursor.length).takeIf { it != BreakIterator.DONE } ?: 0
        val codePointCount = beforeCursor.codePointCount(clusterStart, beforeCursor.length).coerceAtLeast(1)
        if (!connection.deleteSurroundingTextInCodePoints(codePointCount, 0)) {
            sendDownUpKeyEvents(KeyEvent.KEYCODE_DEL)
        }
    }

    private fun stopBackspaceRepeat() {
        backspaceRepeatHandler.removeCallbacks(backspaceRepeat)
    }

    private fun handleSpace() {
        if (!hasPendingComposition) {
            currentInputConnection?.commitText(" ", 1)
            return
        }
        applyEngineSnapshot(processEngineEvent("""{"type":"space"}"""))
    }

    private fun handleEnter() {
        if (hasPendingComposition) {
            applyEngineSnapshot(processEngineEnter())
            return
        }
        val connection = currentInputConnection ?: return
        val info = currentInputEditorInfo
        if (info == null ||
            info.inputType and InputType.TYPE_MASK_CLASS == InputType.TYPE_NULL ||
            info.imeOptions and EditorInfo.IME_FLAG_NO_ENTER_ACTION != 0
        ) {
            sendDownUpKeyEvents(KeyEvent.KEYCODE_ENTER)
            return
        }
        if (!info.actionLabel.isNullOrEmpty() && info.actionId != EditorInfo.IME_ACTION_UNSPECIFIED) {
            connection.performEditorAction(info.actionId)
            return
        }
        when (val action = info.imeOptions and EditorInfo.IME_MASK_ACTION) {
            EditorInfo.IME_ACTION_UNSPECIFIED,
            EditorInfo.IME_ACTION_NONE -> sendDownUpKeyEvents(KeyEvent.KEYCODE_ENTER)
            else -> connection.performEditorAction(action)
        }
    }

    private fun processEngineEnter(): EngineSnapshot? = processEngineEvent("""{"type":"enter"}""")

    private fun commitCandidate(candidate: RankedCandidate) {
        if (!pipelineReady || pipelineHandle == 0L) return
        // The expanded panel represents one candidate snapshot.  Selecting an
        // item invalidates it, so hide it before requesting the next snapshot.
        if (candidateExpanded) collapseCandidateExpansion()
        invalidateCandidateSnapshot()
        enqueueNativeOperation { handle -> nativeBridge.nativeSelectCandidate(handle, candidate.globalIndex) }
    }

    private fun resetState(clearAccumulated: Boolean) {
        if (clearAccumulated) {
            engineGeneration += 1
            pendingEngineEvents.clear()
            hasPendingComposition = false
            pendingEngineOperationCount = 0
            if (pipelineReady && pipelineHandle != 0L) {
                needsEngineClear = false
                val handle = pipelineHandle
                val generation = engineGeneration
                engineExecutor.execute {
                    if (generation == engineGeneration && pipelineHandle == handle && pipelineReady) {
                        runCatching { nativeBridge.nativeClearComposition(handle) }
                    }
                }
            } else {
                needsEngineClear = true
            }
        }
        lastSnapshot = EngineSnapshot()
        candidateLoadGeneration += 1
        candidateLoading = false
        candidateExpanded = false
        expandedCandidates.clear()
        if (::candidateBar.isInitialized) renderState()
    }

    // ===== UI rendering =====

    private fun renderState() {
        if (!::candidateBar.isInitialized) return
        initializeEngine()
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

    private fun postUpdateCandidates() {
        if (::candidateBar.isInitialized) candidateBar.post {
            if (candidateBar.isAttachedToWindow) {
                initializeEngine()
                renderState()
            }
        }
    }

    private fun initializeEngine() {
        if (!pipelineReady || pipelineHandle == 0L || (!needsEngineClear && pendingEngineEvents.isEmpty())) return
        val handle = pipelineHandle
        val generation = engineGeneration
        if (needsEngineClear) {
            needsEngineClear = false
            engineExecutor.execute {
                if (generation != engineGeneration || pipelineHandle != handle || !pipelineReady) return@execute
                runCatching { nativeBridge.nativeClearComposition(handle) }
                val snapshot = runCatching { nativeBridge.nativeSnapshot(handle)?.let(::parseSnapshot) }.getOrNull()
                candidateLoadHandler.post {
                    if (generation == engineGeneration && pipelineHandle == handle && pipelineReady) {
                        applyEngineSnapshot(snapshot)
                    }
                }
            }
        }
        while (pendingEngineEvents.isNotEmpty()) {
            enqueueEngineEvent(handle, generation, pendingEngineEvents.removeFirst())
        }
    }

    private fun enqueueEngineEvent(handle: Long, generation: Long, eventJson: String) {
        pendingEngineOperationCount += 1
        hasPendingComposition = true
        engineExecutor.execute {
            if (generation != engineGeneration || pipelineHandle != handle || !pipelineReady) return@execute
            val snapshot = runCatching {
                nativeBridge.nativeProcessKey(handle, eventJson)?.let(::parseSnapshot)
            }.getOrNull()
            candidateLoadHandler.post {
                if (generation == engineGeneration && pipelineHandle == handle && pipelineReady) {
                    pendingEngineOperationCount = (pendingEngineOperationCount - 1).coerceAtLeast(0)
                    applyEngineSnapshot(snapshot)
                }
            }
        }
    }

    private fun enqueueNativeOperation(operation: (Long) -> String?) {
        if (!pipelineReady || pipelineHandle == 0L) return
        initializeEngine()
        val handle = pipelineHandle
        val generation = engineGeneration
        pendingEngineOperationCount += 1
        hasPendingComposition = true
        engineExecutor.execute {
            if (generation != engineGeneration || pipelineHandle != handle || !pipelineReady) return@execute
            val snapshot = runCatching { operation(handle)?.let(::parseSnapshot) }.getOrNull()
            candidateLoadHandler.post {
                if (generation == engineGeneration && pipelineHandle == handle && pipelineReady) {
                    pendingEngineOperationCount = (pendingEngineOperationCount - 1).coerceAtLeast(0)
                    applyEngineSnapshot(snapshot)
                }
            }
        }
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
        candidateScroll.post {
            if (candidateScroll.isAttachedToWindow) candidateScroll.scrollTo(0, 0)
        }
    }

    private inner class CandidateView(
        context: android.content.Context,
        private val candidate: RankedCandidate,
        index: Int,
        expanded: Boolean,
    ) : LinearLayout(context) {
        private var downX = 0f
        private var downY = 0f
        private var moved = false

        init {
            orientation = VERTICAL
            setPadding(dp(5), dp(if (expanded) 2 else 1), dp(5), dp(if (expanded) 3 else 1))
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
                setTextSize(TypedValue.COMPLEX_UNIT_DIP, 16f); setTextColor(keyTextColor)
                setTypeface(null, Typeface.NORMAL)
                gravity = android.view.Gravity.START
                maxLines = 1
                setSingleLine(true)
            })

            val meta = buildCandidateMeta(candidate)
            if (meta.isNotEmpty()) {
                val displayMeta = if (expanded) expandedCandidateMeta(meta) else meta
                if (expanded) minimumHeight = dp(37)
                addView(TextView(context).apply {
                    text = displayMeta
                    setTextSize(TypedValue.COMPLEX_UNIT_DIP, 10f); setTextColor(keySecondaryTextColor)
                    gravity = android.view.Gravity.START
                    if (expanded) {
                        maxLines = Int.MAX_VALUE
                        setHorizontallyScrolling(false)
                    } else {
                        maxLines = 1
                        setSingleLine(true)
                    }
                })
            }
            contentDescription = candidate.text + "，" + meta
        }

        override fun onTouchEvent(event: MotionEvent): Boolean {
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    downX = event.x
                    downY = event.y
                    moved = false
                    parent.requestDisallowInterceptTouchEvent(true)
                    return true
                }
                MotionEvent.ACTION_MOVE -> {
                    if (kotlin.math.hypot(event.x - downX, event.y - downY) >= dp(8)) {
                        moved = true
                        parent.requestDisallowInterceptTouchEvent(false)
                    }
                    return true
                }
                MotionEvent.ACTION_UP -> {
                    parent.requestDisallowInterceptTouchEvent(false)
                    if (!moved && kotlin.math.hypot(event.x - downX, event.y - downY) < dp(8)) commitCandidate(candidate)
                    return true
                }
                MotionEvent.ACTION_CANCEL -> { parent.requestDisallowInterceptTouchEvent(false); return true }
            }
            return super.onTouchEvent(event)
        }
    }

    private fun buildCandidateMeta(c: RankedCandidate): String {
        if (!c.comment.isNullOrBlank()) return c.comment
        return c.reading.orEmpty()
    }

    private fun expandedCandidateMeta(meta: String): String = meta.replace(" / ", "/\n")

    private fun renderExpandedCandidates() {
        if (!::candidateExpandedRows.isInitialized) return
        val scrollY = candidateExpandedScroll.scrollY
        val generation = candidateLoadGeneration
        candidateExpandedRows.removeAllViews()
        if (!candidateExpanded) {
            candidateExpansionContainer.visibility = View.GONE
            return
        }
        candidateExpansionContainer.visibility = View.VISIBLE
        candidateExpansionContainer.layoutParams = candidateExpansionContainer.layoutParams.apply {
            val parentHeight = (candidateExpansionContainer.parent as? View)?.height ?: 0
            val topMargin = (this as? FrameLayout.LayoutParams)?.topMargin ?: dp(20)
            height = (parentHeight - topMargin).takeIf { it > 0 } ?: dp(154)
        }
        val availableWidth = candidateExpandedScroll.width - dp(6)
        if (availableWidth <= 0) {
            candidateExpandedScroll.post { renderExpandedCandidates() }
            return
        }
        var row = expandedCandidateRow()
        var usedWidth = 0
        expandedCandidates.forEachIndexed { index, candidate ->
            val width = candidateWidth(candidate)
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
        if (candidateHasMore) {
            candidateExpandedRows.addView(Button(this).apply {
                text = when {
                    candidateLoading -> "正在加载候选…"
                    candidateLoadFailed -> "重试加载候选"
                    else -> "加载更多候选"
                }
                isAllCaps = false
                isEnabled = !candidateLoading
                setTextSize(TypedValue.COMPLEX_UNIT_DIP, 12f)
                setTextColor(keySecondaryTextColor)
                setOnClickListener { loadMoreCandidates() }
            }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(44)))
        }
        candidateExpandedScroll.post {
            if (candidateExpanded && candidateLoadGeneration == generation) candidateExpandedScroll.scrollTo(0, scrollY)
        }
    }

    private fun expandedCandidateRow(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = android.view.Gravity.START or android.view.Gravity.TOP
    }

    private fun candidateWidth(candidate: RankedCandidate): Int {
        val density = resources.displayMetrics.density
        val wordWidth = android.graphics.Paint().apply { textSize = 16f * density }.measureText(candidate.text)
        return maxOf(dp(44), kotlin.math.ceil(wordWidth.toDouble()).toInt() + dp(10))
    }

    private fun toggleCandidateExpansion() {
        if (candidateExpanded) {
            collapseCandidateExpansion()
            return
        }
        if (lastSnapshot.candidates.isEmpty()) return
        candidateExpanded = true
        candidateLoadGeneration += 1
        candidateNextOffset = 0
        candidateHasMore = true
        candidateLoading = false
        candidateLoadFailed = false
        expandedCandidates.clear()
        expandedCandidates += lastSnapshot.candidates
        renderState()
        loadMoreCandidates()
    }

    private fun collapseCandidateExpansion() {
        candidateLoadGeneration += 1
        candidateLoading = false
        candidateExpanded = false
        expandedCandidates.clear()
        renderState()
    }

    private fun loadMoreCandidates() {
        if (!candidateExpanded || candidateLoading || !candidateHasMore || !pipelineReady || pipelineHandle == 0L) return
        candidateLoading = true
        candidateLoadFailed = false
        val generation = ++candidateLoadGeneration
        val handle = pipelineHandle
        val rawInput = lastSnapshot.rawInput
        val schemaId = lastSnapshot.schemaId
        val offset = candidateNextOffset
        renderExpandedCandidates()
        engineExecutor.execute {
            val result = runCatching {
                if (pipelineReady && pipelineHandle == handle) nativeBridge.nativeCandidates(handle, offset, 100)?.let(::JSONObject) else null
            }.getOrNull()
            candidateLoadHandler.post {
                if (!candidateExpanded || candidateLoadGeneration != generation || pipelineHandle != handle ||
                    lastSnapshot.rawInput != rawInput || lastSnapshot.schemaId != schemaId) return@post
                candidateLoading = false
                val candidates = runCatching { result?.let { parseSnapshot(it.toString()).candidates } }.getOrNull()
                if (result == null || candidates == null || result.optInt("offset", -1) != offset ||
                    result.optString("rawInput") != rawInput || result.optString("schemaId") != schemaId ||
                    candidates.size > 100 || candidates.withIndex().any { it.value.globalIndex != offset + it.index }) {
                    candidateLoadFailed = true
                } else {
                    if (offset == 0) expandedCandidates.clear()
                    expandedCandidates += candidates
                    candidateNextOffset = offset + candidates.size
                    candidateHasMore = result.optBoolean("hasMore") && candidates.isNotEmpty()
                }
                renderExpandedCandidates()
            }
        }
    }

    private fun processEngineText(text: String): EngineSnapshot? {
        val escaped = JSONObject.quote(text)
        return processEngineEvent("""{"type":"text","text":$escaped}""")
    }

    private fun processEngineEvent(eventJson: String): EngineSnapshot? {
        invalidateCandidateSnapshot()
        if (!pipelineReady || pipelineHandle == 0L) {
            pendingEngineEvents.addLast(eventJson)
            if (runCatching { JSONObject(eventJson).optString("type") == "text" }.getOrDefault(false)) {
                hasPendingComposition = true
            }
            return null
        }
        initializeEngine()
        if (runCatching { JSONObject(eventJson).optString("type") == "text" }.getOrDefault(false)) {
            hasPendingComposition = true
        }
        enqueueEngineEvent(pipelineHandle, engineGeneration, eventJson)
        return null
    }

    private fun applyEngineSnapshot(snapshot: EngineSnapshot?, render: Boolean = true) {
        if (snapshot == null) return
        val compositionChanged = lastSnapshot.rawInput != snapshot.rawInput ||
            lastSnapshot.schemaId != snapshot.schemaId || !snapshot.commitText.isNullOrEmpty()
        if (!snapshot.commitText.isNullOrEmpty()) {
            val selectionStart = editorSelectionStart
            if (currentInputConnection?.commitText(snapshot.commitText, 1) == true && selectionStart != null) {
                val cursor = selectionStart + snapshot.commitText.length
                expectedSelectionStart = cursor
                expectedSelectionEnd = cursor
            }
        }
        lastSnapshot = snapshot
        hasPendingComposition = snapshot.rawInput.isNotEmpty() || pendingEngineOperationCount > 0
        if (compositionChanged) {
            candidateLoadGeneration += 1
            candidateLoading = false
            candidateExpanded = false
            expandedCandidates.clear()
        }
        if (render && ::candidateBar.isInitialized) {
            renderState()
        }
    }

    private fun invalidateCandidateSnapshot() {
        candidateLoadGeneration += 1
        candidateLoading = false
        candidateExpanded = false
        expandedCandidates.clear()
        lastSnapshot = lastSnapshot.copy(preedit = "", candidates = emptyList(), hasNextPage = false)
        if (::candidateBar.isInitialized) renderState()
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
