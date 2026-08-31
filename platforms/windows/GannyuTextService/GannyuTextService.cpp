#include "gannyu_input.h"

#include <windows.h>
#include <windowsx.h>
#include <ctfutb.h>
#include <msctf.h>
#include <olectl.h>
#include <shlwapi.h>
#include <strsafe.h>

#include <algorithm>
#include <atomic>
#include <cwctype>
#include <iterator>
#include <new>
#include <string>
#include <vector>

#pragma comment(lib, "advapi32.lib")
#pragma comment(lib, "gdi32.lib")
#pragma comment(lib, "ole32.lib")
#pragma comment(lib, "shlwapi.lib")

namespace {

// {7A6B9C3E-4A1F-4D58-8B2E-9A1C7D3A2F11}
static const CLSID CLSID_GannyuTextService =
    {0x7a6b9c3e, 0x4a1f, 0x4d58, {0x8b, 0x2e, 0x9a, 0x1c, 0x7d, 0x3a, 0x2f, 0x11}};
// {7A6B9C3F-4A1F-4D58-8B2E-9A1C7D3A2F11}
static const GUID GannyuProfileGuid =
    {0x7a6b9c3f, 0x4a1f, 0x4d58, {0x8b, 0x2e, 0x9a, 0x1c, 0x7d, 0x3a, 0x2f, 0x11}};
// {7A6B9C40-4A1F-4D58-8B2E-9A1C7D3A2F12}
static const GUID GannyuRegionButtonGuid =
    {0x7a6b9c40, 0x4a1f, 0x4d58, {0x8b, 0x2e, 0x9a, 0x1c, 0x7d, 0x3a, 0x2f, 0x12}};
static constexpr LANGID kLangId = 0x0804;
static constexpr wchar_t kTextServiceDescription[] = L"\u8D63\u8BED\u8F93\u5165\u6CD5";
static constexpr wchar_t kCandidateWindowClass[] = L"GannyuCandidateWindow";
static constexpr wchar_t kStatusBarClass[] = L"GannyuStatusBar";
static constexpr wchar_t kLoadWindowClass[] = L"GannyuLoadWindow";
static constexpr wchar_t kRegionRegistryPath[] = L"Software\\GannyuIME";
static constexpr wchar_t kRegionRegistryValue[] = L"RegionId";
static constexpr wchar_t kToolbarXRegistryValue[] = L"ToolbarX";
static constexpr wchar_t kToolbarYRegistryValue[] = L"ToolbarY";
static constexpr size_t kVisibleCandidateCount = 9;
static constexpr size_t kMaxDisplayCharacters = 30;
static const GUID kSupportedCategories[] = {
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_TIPCAP_COMLESS,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
};

static std::atomic<LONG> g_moduleRefs{0};
static HMODULE g_module = nullptr;

static std::vector<std::string> parseStringField(const std::string &json, const std::string &field) {
    std::vector<std::string> out;
    const std::string marker = "\"" + field + "\":\"";
    size_t cursor = 0;
    while ((cursor = json.find(marker, cursor)) != std::string::npos) {
        cursor += marker.size();
        size_t end = cursor;
        while (end < json.size()) {
            if (json[end] == '"' && (end == 0 || json[end - 1] != '\\')) break;
            ++end;
        }
        if (end >= json.size()) break;
        out.emplace_back(json.substr(cursor, end - cursor));
        cursor = end + 1;
    }
    return out;
}

struct CandidateItem {
    std::wstring text;
    std::wstring annotation;
    std::wstring reading;
    std::wstring mandarinReading;
    ULONG consumedBytes = 0;
};

int ScaleForDpi(int value, UINT dpi) {
    if (dpi == 0) {
        dpi = 96;
    }
    return MulDiv(value, static_cast<int>(dpi), 96);
}

std::wstring Utf8ToWide(const std::string &value) {
    if (value.empty()) {
        return {};
    }
    int length = MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), nullptr, 0);
    if (length <= 0) {
        return {};
    }
    std::wstring output(static_cast<size_t>(length), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), output.data(), length);
    return output;
}

std::string WideToUtf8(const std::wstring &value) {
    if (value.empty()) {
        return {};
    }
    int length = WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
    if (length <= 0) {
        return {};
    }
    std::string output(static_cast<size_t>(length), '\0');
    WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), output.data(), length, nullptr, nullptr);
    return output;
}

std::string JsonUnescape(const std::string &value) {
    std::string output;
    output.reserve(value.size());
    for (size_t index = 0; index < value.size(); ++index) {
        char ch = value[index];
        if (ch != '\\' || index + 1 >= value.size()) {
            output.push_back(ch);
            continue;
        }
        char escaped = value[++index];
        switch (escaped) {
            case '\\':
            case '"':
            case '/':
                output.push_back(escaped);
                break;
            case 'b':
                output.push_back('\b');
                break;
            case 'f':
                output.push_back('\f');
                break;
            case 'n':
                output.push_back('\n');
                break;
            case 'r':
                output.push_back('\r');
                break;
            case 't':
                output.push_back('\t');
                break;
            case 'u': {
                if (index + 4 >= value.size()) {
                    output.append("\\u");
                    break;
                }
                unsigned codepoint = 0;
                bool ok = true;
                for (size_t offset = 1; offset <= 4; ++offset) {
                    char hex = value[index + offset];
                    codepoint <<= 4;
                    if (hex >= '0' && hex <= '9') {
                        codepoint |= static_cast<unsigned>(hex - '0');
                    } else if (hex >= 'a' && hex <= 'f') {
                        codepoint |= static_cast<unsigned>(hex - 'a' + 10);
                    } else if (hex >= 'A' && hex <= 'F') {
                        codepoint |= static_cast<unsigned>(hex - 'A' + 10);
                    } else {
                        ok = false;
                        break;
                    }
                }
                if (!ok) {
                    output.append("\\u");
                    break;
                }
                std::wstring wide(1, static_cast<wchar_t>(codepoint));
                output += WideToUtf8(wide);
                index += 4;
                break;
            }
            default:
                output.push_back(escaped);
                break;
        }
    }
    return output;
}

std::wstring ExtractJsonString(const std::string &scope, const std::string &marker) {
    size_t position = scope.find(marker);
    if (position == std::string::npos) {
        return {};
    }
    size_t start = position + marker.size();
    std::string raw;
    for (size_t index = start; index < scope.size(); ++index) {
        char ch = scope[index];
        if (ch == '\\' && index + 1 < scope.size()) {
            raw.push_back(ch);
            raw.push_back(scope[++index]);
            continue;
        }
        if (ch == '"') {
            break;
        }
        raw.push_back(ch);
    }
    return Utf8ToWide(JsonUnescape(raw));
}

ULONG ExtractJsonUint(const std::string &scope, const std::string &marker) {
    size_t position = scope.find(marker);
    if (position == std::string::npos) {
        return 0;
    }
    size_t start = position + marker.size();
    size_t end = start;
    while (end < scope.size() && scope[end] >= '0' && scope[end] <= '9') {
        ++end;
    }
    if (end == start) {
        return 0;
    }
    return static_cast<ULONG>(std::stoul(scope.substr(start, end - start)));
}

std::vector<CandidateItem> ParseCandidates(const char *json) {
    std::vector<CandidateItem> output;
    if (!json) {
        return output;
    }
    const std::string marker = "\"text\":\"";
    const std::string source(json);
    size_t cursor = 0;
    while ((cursor = source.find(marker, cursor)) != std::string::npos) {
        size_t textStart = cursor + marker.size();
        size_t textEnd = textStart;
        while (textEnd < source.size()) {
            if (source[textEnd] == '"' && source[textEnd - 1] != '\\') {
                break;
            }
            ++textEnd;
        }
        if (textEnd >= source.size()) {
            break;
        }
        size_t next = source.find(marker, textEnd + 1);
        std::string scope = source.substr(textEnd, (next == std::string::npos ? source.size() : next) - textEnd);
        CandidateItem item;
        item.text = Utf8ToWide(JsonUnescape(source.substr(textStart, textEnd - textStart)));
        item.annotation = ExtractJsonString(scope, "\"annotation\":\"");
        item.reading = ExtractJsonString(scope, "\"reading\":\"");
        item.mandarinReading = ExtractJsonString(scope, "\"mandarin_reading\":\"");
        item.consumedBytes = ExtractJsonUint(scope, "\"consumed_bytes\":");
        output.push_back(std::move(item));
        cursor = textEnd + 1;
    }
    return output;
}

std::wstring CandidateNote(const CandidateItem &item) {
    if (!item.annotation.empty()) {
        return item.annotation;
    }
    if (!item.reading.empty() && !item.mandarinReading.empty()) {
        return item.reading + L" / " + item.mandarinReading;
    }
    if (!item.reading.empty()) {
        return item.reading;
    }
    return item.mandarinReading;
}

std::wstring LimitDisplayText(const std::wstring &text) {
    if (text.size() <= kMaxDisplayCharacters) {
        return text;
    }
    return text.substr(0, kMaxDisplayCharacters) + L"...";
}

int MeasureTextWidth(HDC hdc, HFONT font, const std::wstring &text) {
    if (!hdc || !font || text.empty()) {
        return 0;
    }
    HGDIOBJ oldFont = SelectObject(hdc, font);
    SIZE size{};
    GetTextExtentPoint32W(hdc, text.c_str(), static_cast<int>(text.size()), &size);
    SelectObject(hdc, oldFont);
    return size.cx;
}

bool TryTranslatePrintableKey(WPARAM key, wchar_t *outChar) {
    if (!outChar) {
        return false;
    }
    BYTE keyboardState[256] = {};
    if (!GetKeyboardState(keyboardState)) {
        return false;
    }
    wchar_t translated[4] = {};
    int written = ToUnicode(static_cast<UINT>(key), MapVirtualKeyW(static_cast<UINT>(key), MAPVK_VK_TO_VSC), keyboardState, translated, 4, 0);
    if (written == 1 && translated[0] >= 0x20) {
        *outChar = translated[0];
        return true;
    }
    return false;
}

const wchar_t *FullwidthSymbolFor(wchar_t ch) {
    switch (ch) {
        case L',':
            return L"\xFF0C";
        case L'.':
            return L"\x3002";
        case L'\\':
            return L"\x3001";
        case L';':
            return L"\xFF1B";
        case L':':
            return L"\xFF1A";
        case L'?':
            return L"\xFF1F";
        case L'!':
            return L"\xFF01";
        case L'(':
            return L"\xFF08";
        case L')':
            return L"\xFF09";
        case L'[':
            return L"\x3010";
        case L']':
            return L"\x3011";
        case L'<':
            return L"\x300A";
        case L'>':
            return L"\x300B";
        case L'"':
            return L"\x201C";
        case L'~':
            return L"\xFF5E";
        case L'-':
            return L"\xFF0D";
        default:
            return nullptr;
    }
}

void ReleaseUnknown(IUnknown *value) {
    if (value) {
        value->Release();
    }
}

class InsertTextEditSession final : public ITfEditSession {
public:
    InsertTextEditSession(ITfContext *context, std::wstring text) : refs_(1), context_(context), text_(std::move(text)) {
        if (context_) {
            context_->AddRef();
        }
    }

    ~InsertTextEditSession() {
        ReleaseUnknown(context_);
    }

    STDMETHODIMP QueryInterface(REFIID riid, void **ppv) override {
        if (!ppv) {
            return E_POINTER;
        }
        *ppv = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfEditSession)) {
            *ppv = static_cast<ITfEditSession *>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return static_cast<ULONG>(InterlockedIncrement(&refs_)); }

    STDMETHODIMP_(ULONG) Release() override {
        LONG refs = InterlockedDecrement(&refs_);
        if (refs == 0) {
            delete this;
        }
        return static_cast<ULONG>(refs);
    }

    STDMETHODIMP DoEditSession(TfEditCookie editCookie) override {
        if (!context_) {
            return E_FAIL;
        }
        ITfInsertAtSelection *insert = nullptr;
        HRESULT hr = context_->QueryInterface(IID_ITfInsertAtSelection, reinterpret_cast<void **>(&insert));
        if (FAILED(hr) || !insert) {
            return FAILED(hr) ? hr : E_FAIL;
        }
        ITfRange *range = nullptr;
        hr = insert->InsertTextAtSelection(
            editCookie,
            TF_IAS_NO_DEFAULT_COMPOSITION,
            text_.c_str(),
            static_cast<LONG>(text_.size()),
            &range
        );
        ReleaseUnknown(range);
        insert->Release();
        return hr;
    }

private:
    LONG refs_;
    ITfContext *context_;
    std::wstring text_;
};

class SelectionRectEditSession final : public ITfEditSession {
public:
    SelectionRectEditSession(ITfContext *context, RECT *rect, bool *hasRect) : refs_(1), context_(context), rect_(rect), hasRect_(hasRect) {
        if (context_) {
            context_->AddRef();
        }
        if (hasRect_) {
            *hasRect_ = false;
        }
    }

    ~SelectionRectEditSession() {
        ReleaseUnknown(context_);
    }

    STDMETHODIMP QueryInterface(REFIID riid, void **ppv) override {
        if (!ppv) {
            return E_POINTER;
        }
        *ppv = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfEditSession)) {
            *ppv = static_cast<ITfEditSession *>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return static_cast<ULONG>(InterlockedIncrement(&refs_)); }

    STDMETHODIMP_(ULONG) Release() override {
        LONG refs = InterlockedDecrement(&refs_);
        if (refs == 0) {
            delete this;
        }
        return static_cast<ULONG>(refs);
    }

    STDMETHODIMP DoEditSession(TfEditCookie editCookie) override {
        if (!context_ || !rect_ || !hasRect_) {
            return E_FAIL;
        }

        TF_SELECTION selection{};
        ULONG fetched = 0;
        HRESULT hr = context_->GetSelection(editCookie, TF_DEFAULT_SELECTION, 1, &selection, &fetched);
        if (FAILED(hr) || fetched != 1 || !selection.range) {
            return FAILED(hr) ? hr : E_FAIL;
        }

        ITfContextView *view = nullptr;
        hr = context_->GetActiveView(&view);
        if (FAILED(hr) || !view) {
            selection.range->Release();
            return FAILED(hr) ? hr : E_FAIL;
        }

        RECT rect{};
        BOOL clipped = FALSE;
        hr = view->GetTextExt(editCookie, selection.range, &rect, &clipped);
        if (SUCCEEDED(hr)) {
            *rect_ = rect;
            *hasRect_ = true;
        }

        view->Release();
        selection.range->Release();
        return hr;
    }

private:
    LONG refs_;
    ITfContext *context_;
    RECT *rect_;
    bool *hasRect_;
};

class GannyuTextService;
bool LoadToolbarPosition(POINT *point) {
    HKEY key = nullptr; DWORD size = sizeof(DWORD), x = 0, y = 0, type = 0;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kRegionRegistryPath, 0, KEY_READ, &key) != ERROR_SUCCESS) return false;
    const bool ok = RegQueryValueExW(key, kToolbarXRegistryValue, nullptr, &type, reinterpret_cast<BYTE *>(&x), &size) == ERROR_SUCCESS && type == REG_DWORD;
    size = sizeof(DWORD);
    const bool yOk = RegQueryValueExW(key, kToolbarYRegistryValue, nullptr, &type, reinterpret_cast<BYTE *>(&y), &size) == ERROR_SUCCESS && type == REG_DWORD;
    RegCloseKey(key); if (!ok || !yOk) return false; point->x = static_cast<LONG>(x); point->y = static_cast<LONG>(y); return true;
}

void SaveToolbarPosition(POINT point) {
    HKEY key = nullptr; if (RegCreateKeyExW(HKEY_CURRENT_USER, kRegionRegistryPath, 0, nullptr, 0, KEY_WRITE, nullptr, &key, nullptr) != ERROR_SUCCESS) return;
    DWORD x = static_cast<DWORD>(point.x), y = static_cast<DWORD>(point.y);
    RegSetValueExW(key, kToolbarXRegistryValue, 0, REG_DWORD, reinterpret_cast<const BYTE *>(&x), sizeof(x));
    RegSetValueExW(key, kToolbarYRegistryValue, 0, REG_DWORD, reinterpret_cast<const BYTE *>(&y), sizeof(y)); RegCloseKey(key);
}

class GannyuRegionButton;

LRESULT CALLBACK CandidateWindowProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam);
LRESULT CALLBACK StatusBarProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam);
LRESULT CALLBACK LoadWindowProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam);

std::string LoadSavedRegionId() {
    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kRegionRegistryPath, 0, KEY_READ, &key) != ERROR_SUCCESS) {
        return {};
    }
    wchar_t value[128] = {};
    DWORD size = sizeof(value);
    DWORD type = 0;
    const bool ok =
        RegQueryValueExW(
            key,
            kRegionRegistryValue,
            nullptr,
            &type,
            reinterpret_cast<BYTE *>(value),
            &size
        ) == ERROR_SUCCESS &&
        type == REG_SZ;
    RegCloseKey(key);
    return ok ? WideToUtf8(value) : std::string{};
}

void SaveRegionId(const std::string &regionId) {
    HKEY key = nullptr;
    if (RegCreateKeyExW(
            HKEY_CURRENT_USER,
            kRegionRegistryPath,
            0,
            nullptr,
            0,
            KEY_WRITE,
            nullptr,
            &key,
            nullptr
        ) != ERROR_SUCCESS) {
        return;
    }
    const std::wstring wide = Utf8ToWide(regionId);
    RegSetValueExW(
        key,
        kRegionRegistryValue,
        0,
        REG_SZ,
        reinterpret_cast<const BYTE *>(wide.c_str()),
        static_cast<DWORD>((wide.size() + 1) * sizeof(wchar_t))
    );
    RegCloseKey(key);
}

class GannyuRegionButton final : public ITfLangBarItemButton, public ITfSource {
public:
    explicit GannyuRegionButton(GannyuTextService *owner);

    void NotifyUpdate();

    STDMETHODIMP QueryInterface(REFIID riid, void **ppv) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;

    STDMETHODIMP GetInfo(TF_LANGBARITEMINFO *info) override;
    STDMETHODIMP GetStatus(DWORD *status) override;
    STDMETHODIMP Show(BOOL show) override;
    STDMETHODIMP GetTooltipString(BSTR *tooltip) override;

    STDMETHODIMP OnClick(TfLBIClick click, POINT point, const RECT *area) override;
    STDMETHODIMP InitMenu(ITfMenu *menu) override;
    STDMETHODIMP OnMenuSelect(UINT itemId) override;
    STDMETHODIMP GetIcon(HICON *icon) override;
    STDMETHODIMP GetText(BSTR *text) override;

    STDMETHODIMP AdviseSink(REFIID riid, IUnknown *unknown, DWORD *cookie) override;
    STDMETHODIMP UnadviseSink(DWORD cookie) override;

private:
    LONG refs_ = 1;
    GannyuTextService *owner_ = nullptr;
    ITfLangBarItemSink *sink_ = nullptr;
    TF_LANGBARITEMINFO info_{};
    std::vector<std::string> menuRegionIds_;
};

class GannyuTextService : public ITfTextInputProcessorEx, public ITfThreadMgrEventSink, public ITfKeyEventSink {
public:
    GannyuTextService() : refs_(1) {
        g_moduleRefs.fetch_add(1);
        LoadRegions();
        regionId_ = LoadSavedRegionId();
        if (std::find(regionIds_.begin(), regionIds_.end(), regionId_) == regionIds_.end()) {
            regionId_.clear();
        }
        if (regionId_.empty() && !regionIds_.empty()) {
            regionId_ = regionIds_.front();
        }
        if (!regionId_.empty()) {
            SaveRegionId(regionId_);
        }
        EnsurePipeline();
        langBarButton_ = new (std::nothrow) GannyuRegionButton(this);
    }

    ~GannyuTextService() {
        if (langBarButton_) {
            langBarButton_->Release();
            langBarButton_ = nullptr;
        }
        if (pipeline_) {
            gannyu_pipeline_destroy(pipeline_);
        }
        SetActiveContext(nullptr);
        DestroyCandidateWindow();
        g_moduleRefs.fetch_sub(1);
    }

    STDMETHODIMP QueryInterface(REFIID riid, void **ppv) override {
        if (!ppv) {
            return E_POINTER;
        }
        *ppv = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfTextInputProcessor) || IsEqualIID(riid, IID_ITfTextInputProcessorEx)) {
            *ppv = static_cast<ITfTextInputProcessorEx *>(this);
        } else if (IsEqualIID(riid, IID_ITfThreadMgrEventSink)) {
            *ppv = static_cast<ITfThreadMgrEventSink *>(this);
        } else if (IsEqualIID(riid, IID_ITfKeyEventSink)) {
            *ppv = static_cast<ITfKeyEventSink *>(this);
        }
        if (!*ppv) {
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return static_cast<ULONG>(InterlockedIncrement(&refs_)); }

    STDMETHODIMP_(ULONG) Release() override {
        LONG refs = InterlockedDecrement(&refs_);
        if (refs == 0) {
            delete this;
        }
        return static_cast<ULONG>(refs);
    }

    STDMETHODIMP Activate(ITfThreadMgr *mgr, TfClientId clientId) override {
        return ActivateEx(mgr, clientId, 0);
    }

    STDMETHODIMP ActivateEx(ITfThreadMgr *mgr, TfClientId clientId, DWORD flags) override {
        (void)flags;
        if (!mgr) {
            return E_INVALIDARG;
        }
        threadMgr_ = mgr;
        threadMgr_->AddRef();
        clientId_ = clientId;

        ITfSource *source = nullptr;
        if (SUCCEEDED(threadMgr_->QueryInterface(IID_ITfSource, reinterpret_cast<void **>(&source))) && source) {
            source->AdviseSink(IID_ITfThreadMgrEventSink, static_cast<ITfThreadMgrEventSink *>(this), &thmgrCookie_);
            source->Release();
        }

        if (SUCCEEDED(threadMgr_->QueryInterface(IID_ITfKeystrokeMgr, reinterpret_cast<void **>(&keystrokeMgr_))) && keystrokeMgr_) {
            keystrokeMgr_->AdviseKeyEventSink(clientId_, static_cast<ITfKeyEventSink *>(this), TRUE);
        }
        if (langBarButton_ && regionIds_.size() > 1) {
            ITfLangBarItemMgr *langBarMgr = nullptr;
            if (SUCCEEDED(threadMgr_->QueryInterface(IID_ITfLangBarItemMgr, reinterpret_cast<void **>(&langBarMgr))) && langBarMgr) {
                const HRESULT hr = langBarMgr->AddItem(langBarButton_);
                if (SUCCEEDED(hr) || hr == TF_E_ALREADY_EXISTS) {
                    langBarItemAdded_ = true;
                }
                langBarMgr->Release();
            }
        }
        if (EnsureStatusBar()) UpdateStatusBar();
        return S_OK;
    }

    STDMETHODIMP Deactivate() override {
        HideCandidateWindow();
        if (statusWindow_) {
            ShowWindow(statusWindow_, SW_HIDE);
        }
        buffer_.clear();
        candidates_.clear();
        preeditDisplay_.clear();
        selectedIndex_ = 0;
        if (threadMgr_ && langBarButton_ && langBarItemAdded_) {
            ITfLangBarItemMgr *langBarMgr = nullptr;
            if (SUCCEEDED(threadMgr_->QueryInterface(IID_ITfLangBarItemMgr, reinterpret_cast<void **>(&langBarMgr))) && langBarMgr) {
                langBarMgr->RemoveItem(langBarButton_);
                langBarMgr->Release();
            }
            langBarItemAdded_ = false;
        }
        if (keystrokeMgr_) {
            keystrokeMgr_->UnadviseKeyEventSink(clientId_);
            keystrokeMgr_->Release();
            keystrokeMgr_ = nullptr;
        }
        if (threadMgr_) {
            ITfSource *source = nullptr;
            if (SUCCEEDED(threadMgr_->QueryInterface(IID_ITfSource, reinterpret_cast<void **>(&source))) && source) {
                source->UnadviseSink(thmgrCookie_);
                source->Release();
            }
            threadMgr_->Release();
            threadMgr_ = nullptr;
        }
        SetActiveContext(nullptr);
        clientId_ = TF_CLIENTID_NULL;
        thmgrCookie_ = TF_INVALID_COOKIE;
        return S_OK;
    }

    STDMETHODIMP OnInitDocumentMgr(ITfDocumentMgr *) override { return S_OK; }
    STDMETHODIMP OnUninitDocumentMgr(ITfDocumentMgr *) override { return S_OK; }

    STDMETHODIMP OnSetFocus(ITfDocumentMgr *, ITfDocumentMgr *) override {
        if (!buffer_.empty()) {
            Reset();
        }
        return S_OK;
    }

    STDMETHODIMP OnPushContext(ITfContext *) override { return S_OK; }
    STDMETHODIMP OnPopContext(ITfContext *) override { return S_OK; }

    STDMETHODIMP OnSetFocus(BOOL foreground) override {
        if (!foreground) {
            Reset();
        }
        return S_OK;
    }

    STDMETHODIMP OnTestKeyDown(ITfContext *, WPARAM key, LPARAM, BOOL *eaten) override {
        if (!eaten) {
            return E_POINTER;
        }
        *eaten = IsShiftKey(key) || IsPunctuationToggleKey(key) || IsTrackedKey(key) ? TRUE : FALSE;
        return S_OK;
    }

    STDMETHODIMP OnKeyDown(ITfContext *context, WPARAM key, LPARAM, BOOL *eaten) override {
        if (!eaten) {
            return E_POINTER;
        }
        if (IsShiftKey(key)) {
            shiftPressed_ = true;
            shiftUsedWithOtherKey_ = false;
            *eaten = TRUE;
            return S_OK;
        }
        if (shiftPressed_) {
            shiftUsedWithOtherKey_ = true;
        }
        *eaten = HandleKey(context, key) ? TRUE : FALSE;
        return S_OK;
    }

    STDMETHODIMP OnTestKeyUp(ITfContext *, WPARAM key, LPARAM, BOOL *eaten) override {
        if (!eaten) {
            return E_POINTER;
        }
        *eaten = IsShiftKey(key) && shiftPressed_ ? TRUE : FALSE;
        return S_OK;
    }

    STDMETHODIMP OnKeyUp(ITfContext *, WPARAM key, LPARAM, BOOL *eaten) override {
        if (!eaten) {
            return E_POINTER;
        }
        if (IsShiftKey(key) && shiftPressed_) {
            const bool toggle = !shiftUsedWithOtherKey_;
            shiftPressed_ = false;
            shiftUsedWithOtherKey_ = false;
            if (toggle) {
                ToggleEnglishMode();
            }
            *eaten = TRUE;
            return S_OK;
        }
        *eaten = FALSE;
        return S_OK;
    }

    STDMETHODIMP OnPreservedKey(ITfContext *, REFGUID, BOOL *eaten) override {
        if (!eaten) {
            return E_POINTER;
        }
        *eaten = FALSE;
        return S_OK;
    }

    const std::vector<std::string> &RegionIds() const { return regionIds_; }

    std::wstring RegionLabel(size_t index) const {
        if (index < regionLabels_.size()) {
            return Utf8ToWide(regionLabels_[index]);
        }
        if (index < regionIds_.size()) {
            return Utf8ToWide(regionIds_[index]);
        }
        return {};
    }

    std::wstring CurrentRegionLabel() const {
        for (size_t index = 0; index < regionIds_.size(); ++index) {
            if (regionIds_[index] == regionId_) {
                return RegionLabel(index);
            }
        }
        if (!regionLabels_.empty()) {
            return Utf8ToWide(regionLabels_.front());
        }
        return std::wstring(kTextServiceDescription);
    }

    const std::string &CurrentRegionId() const { return regionId_; }

    void SwitchRegion(const std::string &regionId) {
        if (regionId.empty() || regionId == regionId_) {
            return;
        }
        const auto found = std::find(regionIds_.begin(), regionIds_.end(), regionId);
        ShowLoadingWindow(found == regionIds_.end() ? Utf8ToWide(regionId) : RegionLabel(static_cast<size_t>(std::distance(regionIds_.begin(), found))));
        GannyuPipelineHandle *replacement = nullptr;
        const char *requested = regionId.c_str();
        if (gannyu_pipeline_create(nullptr, requested, &replacement) != 0 || !replacement) {
            ShowLoadFailure();
            return;
        }
        if (pipeline_) {
            gannyu_pipeline_destroy(pipeline_);
        }
        pipeline_ = replacement;
        regionId_ = regionId;
        SaveRegionId(regionId_);
        if (statusWindow_) {
            UpdateStatusBar();
        }
        Reset();
        NotifyLangBarItemUpdate();
        HideLoadingWindow();
    }

    void NotifyLangBarItemUpdate() {
        if (langBarButton_) {
            langBarButton_->NotifyUpdate();
        }
    }

    LRESULT HandleCandidateWindowMessage(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam) {
        switch (message) {
            case WM_MOUSEACTIVATE:
                return MA_NOACTIVATE;
            case WM_LBUTTONDOWN:
                OnCandidateClick(GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam));
                return 0;
            case WM_MOUSEMOVE: {
                if (draggingToolbar_) {
                    POINT point; GetCursorPos(&point);
                    SetWindowPos(hwnd, HWND_TOPMOST, point.x - toolbarOffset_.x, point.y - toolbarOffset_.y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                }
                return 0;
            }
            case WM_LBUTTONUP:
                if (draggingToolbar_) { RECT rect; GetWindowRect(hwnd, &rect); SaveToolbarPosition({rect.left, rect.top}); draggingToolbar_ = false; ReleaseCapture(); }
                return 0;
            case WM_PAINT: {
                PAINTSTRUCT paintStruct;
                HDC hdc = BeginPaint(hwnd, &paintStruct);
                PaintCandidateWindow(hdc);
                EndPaint(hwnd, &paintStruct);
                return 0;
            }
            case WM_ERASEBKGND:
                return 1;
            default:
                return DefWindowProcW(hwnd, message, wParam, lParam);
        }
    }

    LRESULT HandleStatusBarMessage(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam) {
        switch (message) {
            case WM_MOUSEACTIVATE:
                return MA_NOACTIVATE;
            case WM_LBUTTONDOWN: {
                UINT dpi = GetDpiForWindow(hwnd);
                if (GET_X_LPARAM(lParam) < ScaleForDpi(32, dpi)) {
                    RECT rect; GetWindowRect(hwnd, &rect);
                    toolbarOffset_ = {GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam)};
                    draggingToolbar_ = true; SetCapture(hwnd); return 0;
                }
                const int x = GET_X_LPARAM(lParam);
                if (x < ScaleForDpi(72, dpi)) {
                    ToggleEnglishMode();
                    return 0;
                }
                if (x < ScaleForDpi(112, dpi)) {
                    TogglePunctuationMode();
                    return 0;
                }
                if (x >= ScaleForDpi(200, dpi) && x < ScaleForDpi(260, dpi)) {
                    HMENU menu = CreatePopupMenu();
                    AppendMenuW(menu, MF_STRING, GANNYU_USER_DATA_WORDS, L"用户词");
                    AppendMenuW(menu, MF_STRING, GANNYU_USER_DATA_FREQUENCIES, L"用户词频");
                    AppendMenuW(menu, MF_STRING, GANNYU_USER_DATA_ALL, L"全部");
                    AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
                    AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, L"用户数据均存储在本地。 ");
                    POINT pt; GetCursorPos(&pt);
                    UINT scope = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, nullptr);
                    DestroyMenu(menu);
                    if (scope && MessageBoxW(hwnd, L"清空后无法恢复。", L"清空用户词库", MB_OKCANCEL | MB_ICONWARNING) == IDOK) {
                        EnsurePipeline();
                        if (pipeline_) gannyu_pipeline_user_data_clear(pipeline_, (int)scope);
                    }
                    return 0;
                }
                if (x < ScaleForDpi(112, dpi) || x >= ScaleForDpi(200, dpi) || regionIds_.size() <= 1) return 0;
                HMENU menu = CreatePopupMenu();
                for (size_t i = 0; i < regionIds_.size(); ++i) {
                    UINT flags = MF_STRING;
                    if (regionIds_[i] == regionId_) flags |= MF_CHECKED;
                    std::wstring label = RegionLabel(i);
                    AppendMenuW(menu, flags, (UINT_PTR)(i + 1), label.c_str());
                }
                POINT pt;
                GetCursorPos(&pt);
                UINT selected = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, nullptr);
                DestroyMenu(menu);
                if (selected >= 1 && selected <= regionIds_.size()) {
                    SwitchRegion(regionIds_[selected - 1]);
                }
                return 0;
            }
            case WM_PAINT: {
                PAINTSTRUCT ps;
                HDC hdc = BeginPaint(hwnd, &ps);
                RECT client;
                GetClientRect(hwnd, &client);
                HBRUSH bg = CreateSolidBrush(RGB(240, 244, 248));
                HBRUSH border = CreateSolidBrush(RGB(180, 190, 200));
                FillRect(hdc, &client, bg);
                FrameRect(hdc, &client, border);
                SetBkMode(hdc, TRANSPARENT);
                UINT dpi = GetDpiForWindow(hwnd);
                HFONT font = CreateFontW(-MulDiv(11, (int)dpi, 72), 0, 0, 0, FW_MEDIUM, FALSE, FALSE, FALSE,
                                        DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                                        DEFAULT_PITCH | FF_DONTCARE, L"Segoe UI");
                SelectObject(hdc, font);
                SetTextColor(hdc, RGB(58, 110, 230));
                std::wstring label = englishMode_ ? L"英" : L"中";
                label += fullwidthPunctuation_ ? L"   全   " : L"   半   ";
                label += CurrentRegionLabel();
                label += L" ▾   词库 ▾   ⚙";
                RECT textRect = client;
                textRect.left += ScaleForDpi(8, dpi);
                textRect.right -= ScaleForDpi(8, dpi);
                DrawTextW(hdc, label.c_str(), -1, &textRect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
                DeleteObject(font);
                DeleteObject(border);
                DeleteObject(bg);
                EndPaint(hwnd, &ps);
                return 0;
            }
            case WM_ERASEBKGND:
                return 1;
            default:
                return DefWindowProcW(hwnd, message, wParam, lParam);
        }
    }

private:
    void LoadRegions() {
        char *json = nullptr;
        if (gannyu_region_list(nullptr, &json) == 0 && json) {
            regionIds_ = parseStringField(json, "id");
            regionLabels_ = parseStringField(json, "name_zh");
            gannyu_string_destroy(json);
        }
    }

    void EnsurePipeline() {
        if (pipeline_) {
            return;
        }
        GannyuPipelineHandle *handle = nullptr;
        const char *requested = regionId_.empty() ? nullptr : regionId_.c_str();
        if (gannyu_pipeline_create(nullptr, requested, &handle) == 0) {
            pipeline_ = handle;
        }
    }

    bool HasBlockedModifiers() const {
        return (GetKeyState(VK_CONTROL) & 0x8000) != 0 || (GetKeyState(VK_MENU) & 0x8000) != 0 ||
               (GetKeyState(VK_LWIN) & 0x8000) != 0 || (GetKeyState(VK_RWIN) & 0x8000) != 0;
    }

    bool IsShiftKey(WPARAM key) const {
        return key == VK_SHIFT || key == VK_LSHIFT || key == VK_RSHIFT;
    }

    bool IsPunctuationToggleKey(WPARAM key) const {
        return key == VK_OEM_PERIOD && (GetKeyState(VK_CONTROL) & 0x8000) != 0 &&
               (GetKeyState(VK_MENU) & 0x8000) == 0 && (GetKeyState(VK_LWIN) & 0x8000) == 0 &&
               (GetKeyState(VK_RWIN) & 0x8000) == 0;
    }

    void ToggleEnglishMode() {
        englishMode_ = !englishMode_;
        Reset();
        if (statusWindow_) {
            InvalidateRect(statusWindow_, nullptr, TRUE);
            UpdateWindow(statusWindow_);
        }
    }

    void TogglePunctuationMode() {
        fullwidthPunctuation_ = !fullwidthPunctuation_;
        if (statusWindow_) {
            InvalidateRect(statusWindow_, nullptr, TRUE);
            UpdateWindow(statusWindow_);
        }
    }

    bool IsPageUpKey(WPARAM key) const {
        return key == VK_PRIOR || key == VK_OEM_COMMA;
    }

    bool IsPageDownKey(WPARAM key) const {
        return key == VK_NEXT || key == VK_OEM_PERIOD;
    }

    size_t CurrentPageStart() const {
        if (candidates_.empty()) {
            return 0;
        }
        return (selectedIndex_ / kVisibleCandidateCount) * kVisibleCandidateCount;
    }

    bool IsTrackedKey(WPARAM key) const {
        if (englishMode_ || HasBlockedModifiers()) {
            return false;
        }
        if (key == VK_ESCAPE || key == VK_BACK) {
            return !buffer_.empty();
        }
        if (key == VK_SPACE || key == VK_RETURN) {
            return !buffer_.empty() || !candidates_.empty();
        }
        if (!candidates_.empty() && ((key >= L'1' && key <= L'9') || (key >= VK_NUMPAD1 && key <= VK_NUMPAD9))) {
            return true;
        }
        if (!candidates_.empty() && (IsPageUpKey(key) || IsPageDownKey(key))) {
            return true;
        }
        if (key == VK_LEFT || key == VK_RIGHT || key == VK_UP || key == VK_DOWN || key == VK_TAB || key == VK_PRIOR || key == VK_NEXT) {
            return !candidates_.empty();
        }
        wchar_t translated = 0;
        if (!TryTranslatePrintableKey(key, &translated)) {
            return false;
        }
        if (FullwidthSymbolFor(translated)) {
            return buffer_.empty();
        }
        if (std::iswalpha(translated)) {
            return true;
        }
        if (!buffer_.empty() && (std::iswdigit(translated) || translated == L'\'' || translated == L'-')) {
            return true;
        }
        return false;
    }

    bool HandleKey(ITfContext *context, WPARAM key) {
        if (IsPunctuationToggleKey(key)) {
            TogglePunctuationMode();
            return true;
        }
        if (!context || HasBlockedModifiers() || englishMode_) {
            return false;
        }
        SetActiveContext(context);

        if (key == VK_ESCAPE) {
            if (buffer_.empty()) {
                return false;
            }
            Reset();
            return true;
        }

        if (key == VK_BACK) {
            if (buffer_.empty()) {
                return false;
            }
            buffer_.pop_back();
            RefreshCandidates();
            return true;
        }

        if (!candidates_.empty()) {
            if (IsPageUpKey(key)) {
                if (selectedIndex_ >= kVisibleCandidateCount) {
                    selectedIndex_ -= kVisibleCandidateCount;
                } else {
                    selectedIndex_ = 0;
                }
                RefreshPreeditDisplay();
                UpdateCandidateWindow();
                return true;
            }
            if (IsPageDownKey(key)) {
                selectedIndex_ = std::min(candidates_.size() - 1, selectedIndex_ + kVisibleCandidateCount);
                RefreshPreeditDisplay();
                UpdateCandidateWindow();
                return true;
            }
            if (key == VK_LEFT || key == VK_UP) {
                if (selectedIndex_ == 0) {
                    selectedIndex_ = candidates_.size() - 1;
                } else {
                    --selectedIndex_;
                }
                UpdateCandidateWindow();
                return true;
            }
            if (key == VK_RIGHT || key == VK_DOWN || key == VK_TAB) {
                selectedIndex_ = (selectedIndex_ + 1) % candidates_.size();
                UpdateCandidateWindow();
                return true;
            }
            if ((key >= L'1' && key <= L'9') || (key >= VK_NUMPAD1 && key <= VK_NUMPAD9)) {
                size_t candidateIndex = key >= VK_NUMPAD1 ? static_cast<size_t>(key - VK_NUMPAD1)
                                                          : static_cast<size_t>(key - L'1');
                size_t absoluteIndex = CurrentPageStart() + candidateIndex;
                if (absoluteIndex < candidates_.size()) {
                    selectedIndex_ = absoluteIndex;
                    CommitSelectedCandidate(context, absoluteIndex);
                    return true;
                }
            }
        }

        if (key == VK_SPACE || key == VK_RETURN) {
            if (!candidates_.empty()) {
                CommitSelectedCandidate(context, selectedIndex_);
                return true;
            }
            if (buffer_.empty()) {
                return false;
            }
            bool committed = CommitText(context, Utf8ToWide(buffer_));
            if (committed) {
                Reset();
            }
            return committed;
        }

        wchar_t translated = 0;
        if (!TryTranslatePrintableKey(key, &translated)) {
            return false;
        }
        if (buffer_.empty() && fullwidthPunctuation_) {
            if (const wchar_t *fullwidth = FullwidthSymbolFor(translated)) {
                return CommitText(context, fullwidth);
            }
        }
        return HandlePrintableKey(translated);
    }

    bool HandlePrintableKey(wchar_t translated) {
        if (std::iswalpha(translated)) {
            buffer_.push_back(static_cast<char>(std::towlower(translated)));
            RefreshCandidates();
            return true;
        }
        if (!buffer_.empty() && (std::iswdigit(translated) || translated == L'\'' || translated == L'-')) {
            buffer_.push_back(static_cast<char>(translated));
            RefreshCandidates();
            return true;
        }
        return false;
    }

    bool CommitText(ITfContext *context, const std::wstring &text) {
        if (!context || text.empty() || clientId_ == TF_CLIENTID_NULL) {
            return false;
        }
        InsertTextEditSession *session = new (std::nothrow) InsertTextEditSession(context, text);
        if (!session) {
            return false;
        }
        HRESULT editResult = E_FAIL;
        HRESULT request = context->RequestEditSession(clientId_, session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE, &editResult);
        session->Release();
        return SUCCEEDED(request) && SUCCEEDED(editResult);
    }

    void CommitSelectedCandidate(ITfContext *context, size_t index) {
        if (!context || index >= candidates_.size()) {
            return;
        }
        CandidateItem item = candidates_[index];
        if (!CommitText(context, item.text)) {
            return;
        }
        if (pipeline_) {
            std::string headword = WideToUtf8(item.text);
            gannyu_pipeline_user_dict_boost(pipeline_, headword.c_str(), nullptr);
        }
        if (!buffer_.empty() && item.consumedBytes > 0 && item.consumedBytes < buffer_.size()) {
            buffer_.erase(0, item.consumedBytes);
            selectedIndex_ = 0;
            RefreshCandidates();
            return;
        }
        Reset();
    }


    void RefreshPreeditDisplay() {
        preeditDisplay_ = Utf8ToWide(buffer_);
        if (buffer_.empty() || !pipeline_) {
            return;
        }
        const size_t displayIndex = candidates_.empty() ? 0 : CurrentPageStart();
        const size_t consumedBytes = candidates_.empty() ? 0 : candidates_[displayIndex].consumedBytes;
        char *formatted = nullptr;
        if (gannyu_pipeline_format_preedit(
                pipeline_, buffer_.c_str(), consumedBytes, &formatted) == 0 && formatted) {
            preeditDisplay_ = Utf8ToWide(formatted);
            gannyu_string_destroy(formatted);
        }
    }

    void RefreshCandidates() {
        candidates_.clear();
        if (!buffer_.empty() && pipeline_) {
            char *json = nullptr;
            if (gannyu_pipeline_retrieve(pipeline_, buffer_.c_str(), &json) == 0 && json) {
                candidates_ = ParseCandidates(json);
                gannyu_string_destroy(json);
            }
        }
        if (candidates_.empty()) {
            selectedIndex_ = 0;
        } else if (selectedIndex_ >= candidates_.size()) {
            selectedIndex_ = 0;
        }
        RefreshPreeditDisplay();
        UpdateCandidateWindow();
    }

    bool TryGetContextViewWindow(HWND *window) const {
        if (!window || !activeContext_) {
            return false;
        }
        ITfContextView *view = nullptr;
        HRESULT hr = activeContext_->GetActiveView(&view);
        if (FAILED(hr) || !view) {
            return false;
        }
        HWND hwnd = nullptr;
        hr = view->GetWnd(&hwnd);
        view->Release();
        if (FAILED(hr) || !hwnd) {
            return false;
        }
        *window = hwnd;
        return true;
    }

    bool TryGetThreadAnchorRect(HWND contextWindow, RECT *rect) const {
        if (!contextWindow || !rect) {
            return false;
        }

        DWORD targetThread = GetWindowThreadProcessId(contextWindow, nullptr);
        if (!targetThread) {
            return false;
        }

        GUITHREADINFO info = {};
        info.cbSize = sizeof(info);
        if (GetGUIThreadInfo(targetThread, &info)) {
            HWND anchorWindow = info.hwndCaret ? info.hwndCaret : (info.hwndFocus ? info.hwndFocus : contextWindow);
            POINT topLeft{info.rcCaret.left, info.rcCaret.top};
            POINT bottomRight{info.rcCaret.right, info.rcCaret.bottom};
            ClientToScreen(anchorWindow, &topLeft);
            ClientToScreen(anchorWindow, &bottomRight);
            if (bottomRight.x > topLeft.x || bottomRight.y > topLeft.y) {
                *rect = RECT{topLeft.x, topLeft.y, bottomRight.x, bottomRight.y};
                return true;
            }
        }

        DWORD currentThread = GetCurrentThreadId();
        bool attached = false;
        if (currentThread != targetThread) {
            attached = AttachThreadInput(currentThread, targetThread, TRUE) != FALSE;
        }

        POINT caret{};
        bool hasCaret = GetCaretPos(&caret) != FALSE;
        if (attached) {
            AttachThreadInput(currentThread, targetThread, FALSE);
        }
        if (hasCaret) {
            ClientToScreen(contextWindow, &caret);
            *rect = RECT{caret.x, caret.y, caret.x + ScaleForDpi(2, 96), caret.y + ScaleForDpi(24, 96)};
            return true;
        }

        RECT windowRect{};
        if (GetWindowRect(contextWindow, &windowRect)) {
            *rect = RECT{windowRect.left, windowRect.top, windowRect.left + ScaleForDpi(24, 96), windowRect.top + ScaleForDpi(24, 96)};
            return true;
        }
        return false;
    }

    RECT AnchorRect() const {
        // Primary: ITfContextView::GetTextExt via edit session (standard TSF method).
        if (activeContext_ && clientId_ != TF_CLIENTID_NULL) {
            RECT contextRect{};
            bool hasContextRect = false;
            SelectionRectEditSession *session = new (std::nothrow) SelectionRectEditSession(activeContext_, &contextRect, &hasContextRect);
            if (session) {
                HRESULT sessionResult = E_FAIL;
                HRESULT request = activeContext_->RequestEditSession(clientId_, session, TF_ES_SYNC | TF_ES_READ, &sessionResult);
                session->Release();
                if (SUCCEEDED(request) && SUCCEEDED(sessionResult) && hasContextRect) {
                    if (contextRect.right <= contextRect.left) {
                        contextRect.right = contextRect.left + ScaleForDpi(2, 96);
                    }
                    if (contextRect.bottom <= contextRect.top) {
                        contextRect.bottom = contextRect.top + ScaleForDpi(24, 96);
                    }
                    return contextRect;
                }
            }
        }

        // Fallback: context window + thread info.
        HWND contextWindow = nullptr;
        if (TryGetContextViewWindow(&contextWindow)) {
            RECT threadRect{};
            if (TryGetThreadAnchorRect(contextWindow, &threadRect)) {
                return threadRect;
            }
        }

        GUITHREADINFO info = {};
        info.cbSize = sizeof(info);
        if (GetGUIThreadInfo(0, &info)) {
            HWND anchorWindow = info.hwndCaret ? info.hwndCaret : info.hwndFocus;
            if (anchorWindow) {
                POINT topLeft{info.rcCaret.left, info.rcCaret.top};
                POINT bottomRight{info.rcCaret.right, info.rcCaret.bottom};
                ClientToScreen(anchorWindow, &topLeft);
                ClientToScreen(anchorWindow, &bottomRight);
                if (bottomRight.x > topLeft.x || bottomRight.y > topLeft.y) {
                    return RECT{topLeft.x, topLeft.y, bottomRight.x, bottomRight.y};
                }
                RECT windowRect{};
                if (GetWindowRect(anchorWindow, &windowRect)) {
                    return RECT{windowRect.left, windowRect.top, windowRect.right, windowRect.top + ScaleForDpi(28, 96)};
                }
            }
        }
        POINT cursor{};
        GetCursorPos(&cursor);
        return RECT{cursor.x, cursor.y, cursor.x + ScaleForDpi(24, 96), cursor.y + ScaleForDpi(24, 96)};
    }

    void EnsureFonts(UINT dpi) {
        if (fontDpi_ == dpi && preeditFont_ && candidateFont_ && annotationFont_) {
            return;
        }
        if (preeditFont_) {
            DeleteObject(preeditFont_);
        }
        if (candidateFont_) {
            DeleteObject(candidateFont_);
        }
        if (annotationFont_) {
            DeleteObject(annotationFont_);
        }
        preeditFont_ = CreateFontW(-MulDiv(14, static_cast<int>(dpi), 72), 0, 0, 0, FW_SEMIBOLD, FALSE, FALSE, FALSE,
                                   DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                                   DEFAULT_PITCH | FF_DONTCARE, L"Segoe UI");
        candidateFont_ = CreateFontW(-MulDiv(11, static_cast<int>(dpi), 72), 0, 0, 0, FW_MEDIUM, FALSE, FALSE, FALSE,
                                     DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                                     DEFAULT_PITCH | FF_DONTCARE, L"Segoe UI");
        annotationFont_ = CreateFontW(-MulDiv(10, static_cast<int>(dpi), 72), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                                      DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                                      DEFAULT_PITCH | FF_DONTCARE, L"Segoe UI");
        fontDpi_ = dpi;
    }

    bool EnsureCandidateWindow() {
        if (candidateWindow_) {
            return true;
        }
        WNDCLASSEXW wc = {};
        wc.cbSize = sizeof(wc);
        wc.lpfnWndProc = CandidateWindowProc;
        wc.hInstance = g_module;
        wc.hCursor = LoadCursorW(nullptr, IDC_ARROW);
        wc.lpszClassName = kCandidateWindowClass;
        RegisterClassExW(&wc);
        candidateWindow_ = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            kCandidateWindowClass,
            L"Gannyu Candidate Window",
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            nullptr,
            nullptr,
            g_module,
            this
        );
        return candidateWindow_ != nullptr;
    }

    bool EnsureStatusBar() {
        if (statusWindow_) {
            return true;
        }
        WNDCLASSEXW wc = {};
        wc.cbSize = sizeof(wc);
        wc.lpfnWndProc = StatusBarProc;
        wc.hInstance = g_module;
        wc.hCursor = LoadCursorW(nullptr, IDC_ARROW);
        wc.lpszClassName = kStatusBarClass;
        RegisterClassExW(&wc);
        statusWindow_ = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            kStatusBarClass,
            L"Gannyu Status Bar",
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            nullptr,
            nullptr,
            g_module,
            this
        );
        return statusWindow_ != nullptr;
    }

    void UpdateStatusBar() {
        if (!statusWindow_) {
            return;
        }
        UINT dpi = GetDpiForWindow(statusWindow_);
        if (dpi == 0) dpi = 96;
        int barW = ScaleForDpi(290, dpi);
        int barH = ScaleForDpi(26, dpi);
        RECT workArea{};
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &workArea, 0);
        POINT saved{};
        RECT candidateRect = {workArea.right - barW - ScaleForDpi(16, dpi), workArea.bottom - barH - ScaleForDpi(16, dpi), workArea.right - ScaleForDpi(16, dpi), workArea.bottom - ScaleForDpi(16, dpi)};
        if (LoadToolbarPosition(&saved)) {
            HMONITOR monitor = MonitorFromPoint(saved, MONITOR_DEFAULTTONEAREST); MONITORINFO info{sizeof(info)};
            if (GetMonitorInfoW(monitor, &info)) workArea = info.rcWork;
            candidateRect = {saved.x, saved.y, saved.x + barW, saved.y + barH};
        }
        if (SystemParametersInfoW(SPI_GETWORKAREA, 0, &workArea, 0)) {
            if (candidateRect.right > workArea.right) candidateRect = {workArea.right - barW, candidateRect.top, workArea.right, candidateRect.bottom};
            if (candidateRect.left < workArea.left) candidateRect = {workArea.left, candidateRect.top, workArea.left + barW, candidateRect.bottom};
            if (candidateRect.top < workArea.top) {
                candidateRect = {candidateRect.left, workArea.top, candidateRect.left + barW, workArea.top + barH};
            }
        }
        SetWindowPos(statusWindow_, HWND_TOPMOST, candidateRect.left, candidateRect.top, barW, barH, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(statusWindow_, nullptr, TRUE);
        UpdateWindow(statusWindow_);
    }

    void DestroyStatusBar() {
        if (statusWindow_) {
            DestroyWindow(statusWindow_);
            statusWindow_ = nullptr;
        }
    }

    void UpdateCandidateWindow() {
        if (buffer_.empty()) {
            HideCandidateWindow();
            return;
        }
        if (!EnsureCandidateWindow()) {
            return;
        }
        UINT dpi = GetDpiForWindow(candidateWindow_);
        if (dpi == 0) {
            dpi = 96;
        }
        EnsureFonts(dpi);

        RECT workArea{};
        if (!SystemParametersInfoW(SPI_GETWORKAREA, 0, &workArea, 0)) {
            workArea = RECT{0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)};
        }

        const int outerPadding = ScaleForDpi(12, dpi);
        const int preeditHeight = ScaleForDpi(28, dpi);
        const int candidateHeight = ScaleForDpi(34, dpi);
        const int candidateGap = ScaleForDpi(6, dpi);
        const int sectionGap = candidates_.empty() ? 0 : ScaleForDpi(10, dpi);
        const int minWindowWidth = ScaleForDpi(280, dpi);
        const int availableWorkAreaWidth = static_cast<int>(workArea.right - workArea.left);
        const int maxWindowWidth = std::max(minWindowWidth, availableWorkAreaWidth - ScaleForDpi(24, dpi));
        const int candidateMinWidth = ScaleForDpi(72, dpi);
        const int numberPaddingLeft = ScaleForDpi(10, dpi);
        const int numberSlotWidth = ScaleForDpi(18, dpi);
        const int numberGap = ScaleForDpi(6, dpi);
        const int noteGap = ScaleForDpi(8, dpi);
        const int itemPaddingRight = ScaleForDpi(10, dpi);
        const int innerWidthLimit = maxWindowWidth - outerPadding * 2;
        const int candidateMaxWidth = innerWidthLimit;

        size_t pageStart = CurrentPageStart();
        size_t pageEnd = std::min(candidates_.size(), pageStart + kVisibleCandidateCount);
        int contentWidth = minWindowWidth - outerPadding * 2;
        const std::wstring &preedit = preeditDisplay_;

        HDC measureHdc = GetDC(candidateWindow_);
        if (measureHdc) {
            contentWidth = std::max(contentWidth, MeasureTextWidth(measureHdc, preeditFont_, preedit) + ScaleForDpi(8, dpi));
            for (size_t index = pageStart; index < pageEnd; ++index) {
                std::wstring text = LimitDisplayText(candidates_[index].text);
                std::wstring note = CandidateNote(candidates_[index]);
                int textWidth = MeasureTextWidth(measureHdc, candidateFont_, text);
                int itemWidth = numberPaddingLeft + numberSlotWidth + numberGap + textWidth + itemPaddingRight;
                if (!note.empty()) {
                    itemWidth += noteGap + MeasureTextWidth(measureHdc, annotationFont_, note);
                }
                itemWidth = std::clamp(itemWidth, candidateMinWidth, candidateMaxWidth);
                contentWidth = std::max(contentWidth, itemWidth);
            }
            ReleaseDC(candidateWindow_, measureHdc);
        }

        const int windowWidth = std::clamp(contentWidth + outerPadding * 2, minWindowWidth, maxWindowWidth);
        preeditRect_ = RECT{outerPadding, outerPadding, windowWidth - outerPadding, outerPadding + preeditHeight};
        candidateRects_.clear();
        int currentTop = preeditRect_.bottom + sectionGap;
        const int itemWidth = windowWidth - outerPadding * 2;
        for (size_t index = pageStart; index < pageEnd; ++index) {
            candidateRects_.push_back(RECT{outerPadding, currentTop, outerPadding + itemWidth, currentTop + candidateHeight});
            currentTop += candidateHeight + candidateGap;
        }
        popupSize_.cx = windowWidth;
        popupSize_.cy = candidates_.empty() ? preeditRect_.bottom + outerPadding : currentTop - candidateGap + outerPadding;

        RECT anchor = AnchorRect();
        int x = anchor.left;
        int y = anchor.bottom + ScaleForDpi(8, dpi);
        if (x + popupSize_.cx > workArea.right) {
            x = workArea.right - popupSize_.cx;
        }
        if (x < workArea.left) {
            x = workArea.left;
        }
        if (y + popupSize_.cy > workArea.bottom) {
            y = anchor.top - popupSize_.cy - ScaleForDpi(8, dpi);
        }
        if (y < workArea.top) {
            y = workArea.top;
        }

        SetWindowPos(candidateWindow_, HWND_TOPMOST, x, y, popupSize_.cx, popupSize_.cy, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(candidateWindow_, nullptr, TRUE);
        UpdateWindow(candidateWindow_);
        if (EnsureStatusBar()) UpdateStatusBar();
    }

    void HideCandidateWindow() {
        if (candidateWindow_) {
            ShowWindow(candidateWindow_, SW_HIDE);
        }
    }

    void PaintCandidateWindow(HDC hdc) {
        RECT client{};
        GetClientRect(candidateWindow_, &client);

        const int horizontalInset = ScaleForDpi(10, fontDpi_);
        const int numberPaddingLeft = ScaleForDpi(10, fontDpi_);
        const int numberSlotWidth = ScaleForDpi(18, fontDpi_);
        const int numberGap = ScaleForDpi(6, fontDpi_);
        const int noteGap = ScaleForDpi(8, fontDpi_);
        const int itemPaddingRight = ScaleForDpi(10, fontDpi_);
        HBRUSH backgroundBrush = CreateSolidBrush(RGB(255, 255, 255));
        HBRUSH selectedBrush = CreateSolidBrush(RGB(236, 243, 255));
        HBRUSH borderBrush = CreateSolidBrush(RGB(214, 219, 226));
        HBRUSH separatorBrush = CreateSolidBrush(RGB(232, 235, 240));
        HBRUSH accentBrush = CreateSolidBrush(RGB(58, 110, 230));

        FillRect(hdc, &client, backgroundBrush);
        FrameRect(hdc, &client, borderBrush);

        SetBkMode(hdc, TRANSPARENT);
        SelectObject(hdc, preeditFont_);
        SetTextColor(hdc, RGB(32, 36, 40));

        RECT preeditText = preeditRect_;
        preeditText.left += horizontalInset;
        preeditText.right -= horizontalInset;
        const std::wstring &preedit = preeditDisplay_;
        DrawTextW(hdc, preedit.c_str(), -1, &preeditText, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX);

        if (!candidateRects_.empty()) {
            RECT separator{horizontalInset, preeditRect_.bottom + ScaleForDpi(4, fontDpi_),
                           client.right - horizontalInset, preeditRect_.bottom + ScaleForDpi(5, fontDpi_)};
            FillRect(hdc, &separator, separatorBrush);
        }

        size_t pageStart = CurrentPageStart();
        for (size_t index = 0; index < candidateRects_.size(); ++index) {
            size_t absoluteIndex = pageStart + index;
            RECT row = candidateRects_[index];
            bool selected = absoluteIndex == selectedIndex_;
            if (selected) {
                FillRect(hdc, &row, selectedBrush);
            }

            RECT numberRect = row;
            numberRect.left += numberPaddingLeft;
            numberRect.right = numberRect.left + numberSlotWidth;
            SelectObject(hdc, candidateFont_);
            SetTextColor(hdc, selected ? RGB(36, 90, 235) : RGB(124, 130, 138));
            std::wstring number = std::to_wstring(index + 1);
            DrawTextW(hdc, number.c_str(), -1, &numberRect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);

            RECT textRect = row;
            textRect.left = numberRect.right + numberGap;
            textRect.right = row.right - itemPaddingRight;
            std::wstring text = LimitDisplayText(candidates_[absoluteIndex].text);
            std::wstring note = CandidateNote(candidates_[absoluteIndex]);
            int noteWidth = 0;
            if (!note.empty()) {
                int textWidth = MeasureTextWidth(hdc, candidateFont_, text);
                int measuredNoteWidth = MeasureTextWidth(hdc, annotationFont_, note);
                int remainingNoteWidth = row.right - itemPaddingRight - (textRect.left + textWidth + noteGap);
                noteWidth = std::max(0, std::min(measuredNoteWidth, remainingNoteWidth));
                if (noteWidth > 0) {
                    textRect.right -= noteWidth + noteGap;
                }
            }
            textRect.right = std::max(textRect.right, textRect.left + ScaleForDpi(28, fontDpi_));
            SelectObject(hdc, candidateFont_);
            SetTextColor(hdc, RGB(24, 28, 32));
            DrawTextW(hdc, text.c_str(), -1, &textRect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);

            if (noteWidth > 0) {
                RECT noteRect = row;
                noteRect.left = textRect.right + noteGap;
                noteRect.right = row.right - itemPaddingRight;
                if (noteRect.left < noteRect.right) {
                    SelectObject(hdc, annotationFont_);
                    SetTextColor(hdc, RGB(114, 120, 128));
                    DrawTextW(hdc, note.c_str(), -1, &noteRect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                }
            }

            if (selected) {
                RECT accentLine = row;
                accentLine.top = row.bottom - ScaleForDpi(3, fontDpi_);
                FillRect(hdc, &accentLine, accentBrush);
            }
        }

        DeleteObject(accentBrush);
        DeleteObject(separatorBrush);
        DeleteObject(borderBrush);
        DeleteObject(selectedBrush);
        DeleteObject(backgroundBrush);
    }

    void OnCandidateClick(int x, int y) {
        if (!activeContext_) {
            return;
        }
        POINT point{x, y};
        size_t pageStart = CurrentPageStart();
        for (size_t index = 0; index < candidateRects_.size(); ++index) {
            if (PtInRect(&candidateRects_[index], point)) {
                size_t absoluteIndex = pageStart + index;
                selectedIndex_ = absoluteIndex;
                CommitSelectedCandidate(activeContext_, absoluteIndex);
                return;
            }
        }
    }

    void Reset() {
        buffer_.clear();
        candidates_.clear();
        preeditDisplay_.clear();
        selectedIndex_ = 0;
        HideCandidateWindow();
    }

    void DestroyCandidateWindow() {
        if (loadWindow_) {
            DestroyWindow(loadWindow_);
            loadWindow_ = nullptr;
        }
        if (candidateWindow_) {
            DestroyWindow(candidateWindow_);
            candidateWindow_ = nullptr;
        }
        DestroyStatusBar();
        if (preeditFont_) {
            DeleteObject(preeditFont_);
            preeditFont_ = nullptr;
        }
        if (candidateFont_) {
            DeleteObject(candidateFont_);
            candidateFont_ = nullptr;
        }
        if (annotationFont_) {
            DeleteObject(annotationFont_);
            annotationFont_ = nullptr;
        }
        fontDpi_ = 0;
    }


    bool EnsureLoadWindow() {
        if (loadWindow_) {
            return true;
        }
        WNDCLASSEXW wc = {};
        wc.cbSize = sizeof(wc);
        wc.lpfnWndProc = LoadWindowProc;
        wc.hInstance = g_module;
        wc.hCursor = LoadCursorW(nullptr, IDC_WAIT);
        wc.lpszClassName = kLoadWindowClass;
        RegisterClassExW(&wc);
        loadWindow_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST, kLoadWindowClass,
                                      L"", WS_POPUP, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
                                      nullptr, nullptr, g_module, nullptr);
        return loadWindow_ != nullptr;
    }

    void ShowLoadingWindow(const std::wstring &region) {
        loadText_ = L"正在加载" + region + L"词库…";
        if (!EnsureLoadWindow()) {
            return;
        }
        SetWindowTextW(loadWindow_, loadText_.c_str());
        POINT cursor{};
        GetCursorPos(&cursor);
        SetWindowPos(loadWindow_, HWND_TOPMOST, cursor.x + ScaleForDpi(12, 96), cursor.y + ScaleForDpi(12, 96),
                     ScaleForDpi(260, 96), ScaleForDpi(74, 96), SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(loadWindow_, nullptr, TRUE);
        UpdateWindow(loadWindow_);
    }

    void ShowLoadFailure() {
        loadText_ = L"词库加载失败，仍使用" + CurrentRegionLabel();
        if (!loadWindow_) {
            return;
        }
        SetWindowTextW(loadWindow_, loadText_.c_str());
        InvalidateRect(loadWindow_, nullptr, TRUE);
        UpdateWindow(loadWindow_);
        SetTimer(loadWindow_, 1, 1800, nullptr);
    }

    void HideLoadingWindow() {
        if (loadWindow_) {
            KillTimer(loadWindow_, 1);
            ShowWindow(loadWindow_, SW_HIDE);
        }
    }

    void SetActiveContext(ITfContext *context) {
        if (activeContext_ == context) {
            return;
        }
        if (context) {
            context->AddRef();
        }
        ReleaseUnknown(activeContext_);
        activeContext_ = context;
    }

    LONG refs_;
    ITfThreadMgr *threadMgr_ = nullptr;
    ITfKeystrokeMgr *keystrokeMgr_ = nullptr;
    ITfContext *activeContext_ = nullptr;
    TfClientId clientId_ = TF_CLIENTID_NULL;
    DWORD thmgrCookie_ = TF_INVALID_COOKIE;
    GannyuPipelineHandle *pipeline_ = nullptr;
    GannyuRegionButton *langBarButton_ = nullptr;
    bool langBarItemAdded_ = false;
    std::string regionId_;
    std::vector<std::string> regionIds_;
    std::vector<std::string> regionLabels_;
    std::string buffer_;
    std::wstring preeditDisplay_;
    std::vector<CandidateItem> candidates_;
    size_t selectedIndex_ = 0;
    std::vector<RECT> candidateRects_;
    HWND candidateWindow_ = nullptr;
    HWND statusWindow_ = nullptr;
    HWND loadWindow_ = nullptr;
    std::wstring loadText_;
    bool englishMode_ = false;
    bool fullwidthPunctuation_ = true;
    bool shiftPressed_ = false;
    bool shiftUsedWithOtherKey_ = false;
    bool draggingToolbar_ = false;
    POINT toolbarOffset_{};
    HFONT preeditFont_ = nullptr;
    HFONT candidateFont_ = nullptr;
    HFONT annotationFont_ = nullptr;
    UINT fontDpi_ = 0;
    RECT preeditRect_{};
    SIZE popupSize_{};
};

GannyuRegionButton::GannyuRegionButton(GannyuTextService *owner) : owner_(owner) {
    info_.clsidService = CLSID_GannyuTextService;
    info_.guidItem = GannyuRegionButtonGuid;
    info_.dwStyle = TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY;
    info_.ulSort = 0;
    StringCchCopyW(info_.szDescription, _countof(info_.szDescription), L"Gannyu Region");
}

void GannyuRegionButton::NotifyUpdate() {
    if (sink_) {
        sink_->OnUpdate(TF_LBI_TEXT | TF_LBI_STATUS);
    }
}

STDMETHODIMP GannyuRegionButton::QueryInterface(REFIID riid, void **ppv) {
    if (!ppv) {
        return E_POINTER;
    }
    *ppv = nullptr;
    if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfLangBarItem) ||
        IsEqualIID(riid, IID_ITfLangBarItemButton)) {
        *ppv = static_cast<ITfLangBarItemButton *>(this);
    } else if (IsEqualIID(riid, IID_ITfSource)) {
        *ppv = static_cast<ITfSource *>(this);
    }
    if (!*ppv) {
        return E_NOINTERFACE;
    }
    AddRef();
    return S_OK;
}

STDMETHODIMP_(ULONG) GannyuRegionButton::AddRef() {
    return static_cast<ULONG>(InterlockedIncrement(&refs_));
}

STDMETHODIMP_(ULONG) GannyuRegionButton::Release() {
    LONG refs = InterlockedDecrement(&refs_);
    if (refs == 0) {
        if (sink_) {
            sink_->Release();
        }
        delete this;
    }
    return static_cast<ULONG>(refs);
}

STDMETHODIMP GannyuRegionButton::GetInfo(TF_LANGBARITEMINFO *info) {
    if (!info) {
        return E_POINTER;
    }
    *info = info_;
    return S_OK;
}

STDMETHODIMP GannyuRegionButton::GetStatus(DWORD *status) {
    if (!status) {
        return E_POINTER;
    }
    *status = 0;
    return S_OK;
}

STDMETHODIMP GannyuRegionButton::Show(BOOL) { return S_OK; }

STDMETHODIMP GannyuRegionButton::GetTooltipString(BSTR *tooltip) {
    if (!tooltip) {
        return E_POINTER;
    }
    *tooltip = SysAllocString(L"切换赣语地区");
    return *tooltip ? S_OK : E_OUTOFMEMORY;
}

STDMETHODIMP GannyuRegionButton::OnClick(TfLBIClick, POINT, const RECT *) { return S_OK; }

STDMETHODIMP GannyuRegionButton::InitMenu(ITfMenu *menu) {
    if (!menu || !owner_) {
        return E_INVALIDARG;
    }
    menuRegionIds_.clear();
    const auto &ids = owner_->RegionIds();
    for (size_t index = 0; index < ids.size(); ++index) {
        const std::wstring label = owner_->RegionLabel(index);
        const DWORD flags = ids[index] == owner_->CurrentRegionId() ? TF_LBMENUF_CHECKED : 0;
        menu->AddMenuItem(
            static_cast<UINT>(index),
            flags,
            nullptr,
            nullptr,
            label.c_str(),
            static_cast<ULONG>(label.size()),
            nullptr
        );
        menuRegionIds_.push_back(ids[index]);
    }
    return S_OK;
}

STDMETHODIMP GannyuRegionButton::OnMenuSelect(UINT itemId) {
    if (owner_ && itemId < menuRegionIds_.size()) {
        owner_->SwitchRegion(menuRegionIds_[itemId]);
    }
    return S_OK;
}

STDMETHODIMP GannyuRegionButton::GetIcon(HICON *icon) {
    if (!icon) {
        return E_POINTER;
    }
    *icon = nullptr;
    return E_FAIL;
}

STDMETHODIMP GannyuRegionButton::GetText(BSTR *text) {
    if (!text) {
        return E_POINTER;
    }
    const std::wstring label = owner_ ? owner_->CurrentRegionLabel() : std::wstring(kTextServiceDescription);
    *text = SysAllocString(label.c_str());
    return *text ? S_OK : E_OUTOFMEMORY;
}

STDMETHODIMP GannyuRegionButton::AdviseSink(REFIID riid, IUnknown *unknown, DWORD *cookie) {
    if (!cookie) {
        return E_POINTER;
    }
    if (!IsEqualIID(riid, IID_ITfLangBarItemSink) || !unknown) {
        return E_NOINTERFACE;
    }
    if (sink_) {
        return CONNECT_E_ADVISELIMIT;
    }
    HRESULT hr = unknown->QueryInterface(IID_ITfLangBarItemSink, reinterpret_cast<void **>(&sink_));
    if (FAILED(hr)) {
        sink_ = nullptr;
        return CONNECT_E_CANNOTCONNECT;
    }
    *cookie = 1;
    return S_OK;
}

STDMETHODIMP GannyuRegionButton::UnadviseSink(DWORD cookie) {
    if (cookie != 1 || !sink_) {
        return CONNECT_E_NOCONNECTION;
    }
    sink_->Release();
    sink_ = nullptr;
    return S_OK;
}

LRESULT CALLBACK CandidateWindowProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam) {
    if (message == WM_NCCREATE) {
        auto *createStruct = reinterpret_cast<CREATESTRUCTW *>(lParam);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(createStruct->lpCreateParams));
    }
    auto *service = reinterpret_cast<GannyuTextService *>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
    if (service) {
        return service->HandleCandidateWindowMessage(hwnd, message, wParam, lParam);
    }
    return DefWindowProcW(hwnd, message, wParam, lParam);
}

LRESULT CALLBACK StatusBarProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam) {
    if (message == WM_NCCREATE) {
        auto *createStruct = reinterpret_cast<CREATESTRUCTW *>(lParam);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(createStruct->lpCreateParams));
    }
    auto *service = reinterpret_cast<GannyuTextService *>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
    if (service) {
        return service->HandleStatusBarMessage(hwnd, message, wParam, lParam);
    }
    return DefWindowProcW(hwnd, message, wParam, lParam);
}


LRESULT CALLBACK LoadWindowProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam) {
    if (message == WM_TIMER) {
        ShowWindow(hwnd, SW_HIDE);
        KillTimer(hwnd, 1);
        return 0;
    }
    if (message == WM_PAINT) {
        PAINTSTRUCT ps{};
        HDC hdc = BeginPaint(hwnd, &ps);
        RECT client{};
        GetClientRect(hwnd, &client);
        HBRUSH background = CreateSolidBrush(RGB(255, 255, 255));
        HBRUSH border = CreateSolidBrush(RGB(180, 190, 200));
        HBRUSH progress = CreateSolidBrush(RGB(58, 110, 230));
        FillRect(hdc, &client, background);
        FrameRect(hdc, &client, border);
        wchar_t text[256] = {};
        GetWindowTextW(hwnd, text, _countof(text));
        RECT textRect{ScaleForDpi(12, 96), ScaleForDpi(10, 96), client.right - ScaleForDpi(12, 96), ScaleForDpi(34, 96)};
        SetBkMode(hdc, TRANSPARENT);
        DrawTextW(hdc, text, -1, &textRect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        RECT track{ScaleForDpi(12, 96), ScaleForDpi(46, 96), client.right - ScaleForDpi(12, 96), ScaleForDpi(54, 96)};
        FrameRect(hdc, &track, border);
        RECT filled{track.left + 1, track.top + 1, track.left + (track.right - track.left) * 3 / 5, track.bottom - 1};
        FillRect(hdc, &filled, progress);
        DeleteObject(progress);
        DeleteObject(border);
        DeleteObject(background);
        EndPaint(hwnd, &ps);
        return 0;
    }
    return DefWindowProcW(hwnd, message, wParam, lParam);
}

class GannyuClassFactory : public IClassFactory {
public:
    GannyuClassFactory() : refs_(1) { g_moduleRefs.fetch_add(1); }
    ~GannyuClassFactory() { g_moduleRefs.fetch_sub(1); }

    STDMETHODIMP QueryInterface(REFIID riid, void **ppv) override {
        if (!ppv) {
            return E_POINTER;
        }
        *ppv = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_IClassFactory)) {
            *ppv = static_cast<IClassFactory *>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return static_cast<ULONG>(InterlockedIncrement(&refs_)); }

    STDMETHODIMP_(ULONG) Release() override {
        LONG refs = InterlockedDecrement(&refs_);
        if (refs == 0) {
            delete this;
        }
        return static_cast<ULONG>(refs);
    }

    STDMETHODIMP CreateInstance(IUnknown *outer, REFIID riid, void **ppv) override {
        if (outer) {
            return CLASS_E_NOAGGREGATION;
        }
        GannyuTextService *service = new (std::nothrow) GannyuTextService();
        if (!service) {
            return E_OUTOFMEMORY;
        }
        HRESULT hr = service->QueryInterface(riid, ppv);
        service->Release();
        return hr;
    }

    STDMETHODIMP LockServer(BOOL lock) override {
        if (lock) {
            g_moduleRefs.fetch_add(1);
        } else {
            g_moduleRefs.fetch_sub(1);
        }
        return S_OK;
    }

private:
    LONG refs_;
};

HRESULT RegisterComServer(const wchar_t *modulePath) {
    wchar_t clsidText[64] = {};
    if (!StringFromGUID2(CLSID_GannyuTextService, clsidText, static_cast<int>(_countof(clsidText)))) {
        return E_FAIL;
    }

    wchar_t keyPath[256] = {};
    StringCchPrintfW(keyPath, _countof(keyPath), L"CLSID\\%s", clsidText);

    HKEY clsidKey = nullptr;
    if (RegCreateKeyExW(HKEY_CLASSES_ROOT, keyPath, 0, nullptr, 0, KEY_WRITE, nullptr, &clsidKey, nullptr) != ERROR_SUCCESS) {
        return SELFREG_E_CLASS;
    }
    RegSetValueExW(clsidKey, nullptr, 0, REG_SZ, reinterpret_cast<const BYTE *>(kTextServiceDescription), sizeof(kTextServiceDescription));
    RegCloseKey(clsidKey);

    wchar_t inprocPath[320] = {};
    StringCchPrintfW(inprocPath, _countof(inprocPath), L"%s\\InprocServer32", keyPath);
    HKEY inprocKey = nullptr;
    if (RegCreateKeyExW(HKEY_CLASSES_ROOT, inprocPath, 0, nullptr, 0, KEY_WRITE, nullptr, &inprocKey, nullptr) != ERROR_SUCCESS) {
        return SELFREG_E_CLASS;
    }
    RegSetValueExW(inprocKey, nullptr, 0, REG_SZ, reinterpret_cast<const BYTE *>(modulePath), static_cast<DWORD>((wcslen(modulePath) + 1) * sizeof(wchar_t)));
    const wchar_t threadingModel[] = L"Apartment";
    RegSetValueExW(inprocKey, L"ThreadingModel", 0, REG_SZ, reinterpret_cast<const BYTE *>(threadingModel), sizeof(threadingModel));
    RegCloseKey(inprocKey);
    return S_OK;
}

HRESULT RegisterTextServiceProfile(const wchar_t *modulePath) {
    ITfInputProcessorProfiles *profiles = nullptr;
    HRESULT hr = CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_ITfInputProcessorProfiles, reinterpret_cast<void **>(&profiles));
    if (FAILED(hr) || !profiles) {
        return FAILED(hr) ? hr : E_FAIL;
    }

    hr = profiles->Register(CLSID_GannyuTextService);
    if (SUCCEEDED(hr) || hr == TF_E_ALREADY_EXISTS) {
        hr = profiles->AddLanguageProfile(
            CLSID_GannyuTextService,
            kLangId,
            GannyuProfileGuid,
            kTextServiceDescription,
            static_cast<ULONG>(wcslen(kTextServiceDescription)),
            modulePath,
            static_cast<ULONG>(wcslen(modulePath)),
            0
        );
        if (hr == TF_E_ALREADY_EXISTS) {
            hr = S_OK;
        }
        if (SUCCEEDED(hr)) {
            hr = profiles->EnableLanguageProfile(CLSID_GannyuTextService, kLangId, GannyuProfileGuid, TRUE);
        }
    }
    profiles->Release();
    if (FAILED(hr)) {
        return hr;
    }

    ITfCategoryMgr *categories = nullptr;
    hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER,
                          IID_ITfCategoryMgr, reinterpret_cast<void **>(&categories));
    if (FAILED(hr) || !categories) {
        return FAILED(hr) ? hr : E_FAIL;
    }
    for (const GUID &category : kSupportedCategories) {
        hr = categories->RegisterCategory(CLSID_GannyuTextService, category, CLSID_GannyuTextService);
        if (FAILED(hr)) {
            break;
        }
    }
    categories->Release();
    return hr;
}

HRESULT UnregisterTextServiceProfile() {
    ITfCategoryMgr *categories = nullptr;
    HRESULT hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_ITfCategoryMgr, reinterpret_cast<void **>(&categories));
    if (SUCCEEDED(hr) && categories) {
        for (const GUID &category : kSupportedCategories) {
            categories->UnregisterCategory(CLSID_GannyuTextService, category, CLSID_GannyuTextService);
        }
        categories->Release();
    }

    ITfInputProcessorProfiles *profiles = nullptr;
    hr = CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr, CLSCTX_INPROC_SERVER,
                          IID_ITfInputProcessorProfiles, reinterpret_cast<void **>(&profiles));
    if (FAILED(hr) || !profiles) {
        return FAILED(hr) ? hr : E_FAIL;
    }
    profiles->EnableLanguageProfile(CLSID_GannyuTextService, kLangId, GannyuProfileGuid, FALSE);
    profiles->RemoveLanguageProfile(CLSID_GannyuTextService, kLangId, GannyuProfileGuid);
    profiles->Unregister(CLSID_GannyuTextService);
    profiles->Release();

    wchar_t clsidText[64] = {};
    if (!StringFromGUID2(CLSID_GannyuTextService, clsidText, static_cast<int>(_countof(clsidText)))) {
        return E_FAIL;
    }
    wchar_t keyPath[256] = {};
    StringCchPrintfW(keyPath, _countof(keyPath), L"CLSID\\%s", clsidText);
    SHDeleteKeyW(HKEY_CLASSES_ROOT, keyPath);
    return S_OK;
}

}  // namespace

extern "C" {

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_module = module;
        DisableThreadLibraryCalls(module);
    }
    return TRUE;
}

STDAPI DllCanUnloadNow(void) {
    return g_moduleRefs.load() == 0 ? S_OK : S_FALSE;
}

STDAPI DllGetClassObject(REFCLSID clsid, REFIID riid, LPVOID *ppv) {
    if (!IsEqualCLSID(clsid, CLSID_GannyuTextService)) {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    GannyuClassFactory *factory = new (std::nothrow) GannyuClassFactory();
    if (!factory) {
        return E_OUTOFMEMORY;
    }
    HRESULT hr = factory->QueryInterface(riid, ppv);
    factory->Release();
    return hr;
}

STDAPI DllRegisterServer(void) {
    wchar_t modulePath[MAX_PATH] = {};
    if (!GetModuleFileNameW(g_module, modulePath, MAX_PATH)) {
        return SELFREG_E_CLASS;
    }

    HRESULT init = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    bool shouldUninit = SUCCEEDED(init);
    if (FAILED(init) && init != RPC_E_CHANGED_MODE) {
        return init;
    }

    HRESULT hr = RegisterComServer(modulePath);
    if (SUCCEEDED(hr)) {
        hr = RegisterTextServiceProfile(modulePath);
    }

    if (shouldUninit) {
        CoUninitialize();
    }
    return hr;
}

STDAPI DllUnregisterServer(void) {
    HRESULT init = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    bool shouldUninit = SUCCEEDED(init);
    if (FAILED(init) && init != RPC_E_CHANGED_MODE) {
        return init;
    }

    HRESULT hr = UnregisterTextServiceProfile();

    if (shouldUninit) {
        CoUninitialize();
    }
    return hr;
}

}  // extern "C"
