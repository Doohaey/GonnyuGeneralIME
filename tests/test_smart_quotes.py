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

    def test_macos_uses_smart_quote(self):
        source = (ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift").read_text()
        assert "private func smartDoubleQuote(client sender: Any!) -> String" in source
        assert "private static func smartDoubleQuote(for text: String) -> String" in source
        assert "attributedSubstring(from:" in source
        assert "text == \"\\\"\"" in source
        assert "＂" not in source

    def test_windows_uses_smart_quote(self):
        source = (ROOT / "platforms/windows/GannyuTextService/GannyuTextService.cpp").read_text()
        assert "SmartDoubleQuoteForText" in source
        assert "class TextBeforeCursorEditSession" in source
        assert "if (translated == L'\"')" in source
        assert "return buffer_.empty() && fullwidthPunctuation_;" in source
        assert "return CommitText(context, SmartDoubleQuote(context));" in source
        fullwidth = declaration(source, "const wchar_t *FullwidthSymbolFor(wchar_t ch)")
        assert "case L'\"'" not in fullwidth

    def test_linux_uses_smart_quote(self):
        source = (ROOT / "platforms/linux/fcitx5/gannyu_fcitx5.cpp").read_text()
        assert "smartDoubleQuoteForText" in source
        assert "smartDoubleQuote(fcitx::InputContext *ic)" in source
        assert "ic->commitString(smartDoubleQuote(ic));" in source
        fullwidth = declaration(source, "std::string chineseFullwidthSymbol(int sym)")
        assert "case '\"':" not in fullwidth


@unittest.skipUnless(sys.platform == "darwin" and shutil.which("xcrun"), "requires macOS Swift")
class MacSmartQuoteLogicTests(unittest.TestCase):
    def test_smart_double_quote_toggle(self):
        source = (ROOT / "platforms/macos/Sources/GannyuInputMethodHost/GannyuInputController.swift").read_text()
        function = declaration(source, "private static func smartDoubleQuote(for text: String) -> String")
        function = function.replace("private static func", "func", 1)
        harness = f'''
import Foundation
{function}
precondition(smartDoubleQuote(for: "") == "“")
precondition(smartDoubleQuote(for: "你说") == "“")
precondition(smartDoubleQuote(for: "他说：“你好") == "”")
precondition(smartDoubleQuote(for: "他说：“你好”") == "“")
precondition(smartDoubleQuote(for: "第一行\\n他说：“你好") == "”")
precondition(smartDoubleQuote(for: "第一行：”\\n第二行") == "“")
'''
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "SmartQuotes.swift"
            path.write_text(harness)
            binary = Path(directory) / "smart-quotes"
            compiled = subprocess.run(["xcrun", "swiftc", str(path), "-o", str(binary)], capture_output=True, text=True)
            self.assertEqual(compiled.returncode, 0, compiled.stderr)
            executed = subprocess.run([str(binary)], capture_output=True, text=True)
            self.assertEqual(executed.returncode, 0, executed.stderr)
