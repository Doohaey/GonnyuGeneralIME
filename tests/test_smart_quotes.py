import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def declaration(source: str, signature: str) -> str:
    start = source.find(signature)
    if start < 0:
        raise AssertionError(f"missing declaration: {signature}")
    brace = source.find("{", start)
    if brace < 0:
        raise AssertionError(f"missing body: {signature}")
    depth = 0
    for index in range(brace, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start:index + 1]
    raise AssertionError(f"unclosed declaration: {signature}")


class SmartQuoteSourceTests(unittest.TestCase):
    def test_android_uses_explicit_quote_keys(self):
        source = (ROOT / "platforms/android/app/src/main/java/io/gannyu/input/GannyuInputMethodService.kt").read_text()
        assert 'private val NUM_ROW_2 = listOf("－", "／", "：", "；", "（", "）", "￥", "@", "“", "”")' in source
        assert "fun smartDoubleQuoteForText(" not in source
        assert "key.label == \"“\" || key.label == \"”\"" in source
        assert "fun handleSmartDoubleQuote()" not in source
        assert "＂" not in source
        assert "＆" not in source

    def test_macos_uses_alternating_quote(self):
        source = (ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift").read_text()
        assert "private var doubleQuoteOpen = false" in source
        assert "private func nextDoubleQuote() -> String" in source
        assert "private static func nextDoubleQuote(opening: inout Bool) -> String" in source
        assert 'return opening ? "”" : "“"' in source
        assert "attributedSubstring(from:" not in source
        assert "＂" not in source

    def test_windows_uses_alternating_quote(self):
        source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text()
        assert "std::wstring CurrentDoubleQuote() const" in source
        assert "bool CommitDoubleQuote(ITfContext *context)" in source
        assert "bool doubleQuoteOpen_ = false;" in source
        assert r'return doubleQuoteOpen_ ? L"\u201D" : L"\u201C";' in source
        assert "class TextBeforeCursorEditSession" not in source
        assert "SmartDoubleQuoteForText" not in source
        assert "NextDoubleQuote" not in source
        assert "return CommitDoubleQuote(context);" in source
        fullwidth = declaration(source, "const wchar_t *FullwidthSymbolFor(wchar_t ch)")
        assert "case L'\"'" not in fullwidth

    def test_windows_resets_quote_state_only_on_context_change(self):
        source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text()
        set_context = declaration(source, "void SetActiveContext(ITfContext *context)")
        assert "doubleQuoteOpen_ = false;" in set_context
        assert "doubleQuoteOpen_" not in declaration(source, "void ClearInputModel()")
        assert "doubleQuoteOpen_" not in declaration(source, "void Reset(bool cancelComposition = true)")

    def test_windows_toggles_after_successful_commit(self):
        source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text()
        commit = declaration(source, "bool CommitDoubleQuote(ITfContext *context)")
        assert commit.index("CommitText") < commit.index("doubleQuoteOpen_ = !doubleQuoteOpen_")

    def test_linux_uses_alternating_quote(self):
        source = (ROOT / "platforms/linux/fcitx5/gannyu_fcitx5.cpp").read_text()
        assert "std::string nextDoubleQuote()" in source
        assert "bool doubleQuoteOpen = false;" in source
        assert r'return opening ? "\xE2\x80\x9D" : "\xE2\x80\x9C";' in source
        assert "smartDoubleQuoteForText" not in source
        assert "state->nextDoubleQuote()" in source
        assert "ic->commitString(state->nextDoubleQuote());" in source
        fullwidth = declaration(source, "std::string chineseFullwidthSymbol(int sym)")
        assert "case '\"':" not in fullwidth


@unittest.skipUnless(sys.platform == "darwin" and shutil.which("xcrun"), "requires macOS Swift")
class MacQuoteToggleLogicTests(unittest.TestCase):
    def test_alternating_double_quote(self):
        source = (ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift").read_text()
        function = declaration(source, "private static func nextDoubleQuote(opening: inout Bool) -> String")
        function = function.replace("private static func", "func", 1)
        harness = f'''
import Foundation
{function}
var open = false
precondition(nextDoubleQuote(opening: &open) == "“")
precondition(open == true)
precondition(nextDoubleQuote(opening: &open) == "”")
precondition(open == false)
precondition(nextDoubleQuote(opening: &open) == "“")
'''
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "DoubleQuote.swift"
            path.write_text(harness)
            binary = Path(directory) / "double-quote"
            compiled = subprocess.run(["xcrun", "swiftc", str(path), "-o", str(binary)], capture_output=True, text=True)
            self.assertEqual(compiled.returncode, 0, compiled.stderr)
            executed = subprocess.run([str(binary)], capture_output=True, text=True)
            self.assertEqual(executed.returncode, 0, executed.stderr)
