#include "gannyu_input.h"
#include <fcitx/action.h>
#include <fcitx-utils/i18n.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/menu.h>
#include <fcitx/statusarea.h>
#include <fcitx/text.h>
#include <fcitx/userinterfacemanager.h>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <list>
#include <memory>
#include <string>
#include <vector>

namespace {

std::string resolveManifestPath() {
    if (const char *env = std::getenv("GANNYU_MANIFEST"); env && *env) return env;
    // Use embedded encrypted resources (in the .so) by default;
    // set GANNYU_MANIFEST to a filesystem path only for local dev/debug.
    return "";
}

std::vector<std::string> parseStringField(const std::string &json, const std::string &field) {
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

struct RankedItem {
    std::string text;
    std::string comment;
    int consumedBytes = 0;
    std::string reading;
    std::string mandarinReading;
};

fcitx::KeyList selectionKeys() {
    return {
        fcitx::Key(FcitxKey_1),
        fcitx::Key(FcitxKey_2),
        fcitx::Key(FcitxKey_3),
        fcitx::Key(FcitxKey_4),
        fcitx::Key(FcitxKey_5),
        fcitx::Key(FcitxKey_6),
        fcitx::Key(FcitxKey_7),
        fcitx::Key(FcitxKey_8),
        fcitx::Key(FcitxKey_9),
    };
}

std::string buildCandidateDisplay(const RankedItem &item) {
    if (!item.comment.empty()) return item.text + " " + item.comment;
    if (!item.reading.empty()) return item.text + " " + item.reading;
    return item.text;
}

/* Map common ASCII punctuation to Chinese full-width equivalents.
 * Returns a UTF-8 string, or empty if no mapping exists. */
std::string chineseFullwidthSymbol(int sym) {
    switch (sym) {
        case ',':  return "\xEF\xBC\x8C"; /* ，U+FF0C */
        case '.':  return "\xE3\x80\x82"; /* 。U+3002 */
        case '\\': return "\xE3\x80\x81"; /* 、U+3001 */
        case ';':  return "\xEF\xBC\x9B"; /* ；U+FF1B */
        case ':':  return "\xEF\xBC\x9A"; /* ：U+FF1A */
        case '?':  return "\xEF\xBC\x9F"; /* ？U+FF1F */
        case '!':  return "\xEF\xBC\x81"; /* ！U+FF01 */
        case '(':  return "\xEF\xBC\x88"; /* （U+FF08 */
        case ')':  return "\xEF\xBC\x89"; /* ）U+FF09 */
        case '[':  return "\xE3\x80\x90"; /* 【U+3010 */
        case ']':  return "\xE3\x80\x91"; /* 】U+3011 */
        case '<':  return "\xE3\x80\x8A"; /* 《U+300A */
        case '>':  return "\xE3\x80\x8B"; /* 》U+300B */
        case '"':  return "\xE2\x80\x9C"; /* "U+201C left double quote */
        case '~':  return "\xEF\xBD\x9E"; /* ～U+FF5E */
        case '-':  return "\xEF\xBC\x8D"; /* －U+FF0D */
        default:   return {};
    }
}

// Parse the retrieve() JSON array preserving order. Each candidate becomes a
// commit text plus a small-font comment: the annotation field when present,
// or a [官] fallback tag for Mandarin-only words without an annotation.
static std::string extractStringField(const std::string &scope, const std::string &marker) {
    std::string result;
    size_t pos = scope.find(marker);
    if (pos == std::string::npos) return result;
    size_t start = pos + marker.size();
    size_t end = start;
    while (end < scope.size()) {
        if (scope[end] == '"' && scope[end - 1] != '\\') break;
        ++end;
    }
    if (end > start) result = scope.substr(start, end - start);
    return result;
}

std::vector<RankedItem> parseRankedItems(const std::string &json) {
    std::vector<RankedItem> out;
    const std::string textMarker = "\"text\":\"";
    size_t cursor = 0;
    while ((cursor = json.find(textMarker, cursor)) != std::string::npos) {
        size_t textStart = cursor + textMarker.size();
        size_t textEnd = textStart;
        while (textEnd < json.size()) {
            if (json[textEnd] == '"' && json[textEnd - 1] != '\\') break;
            ++textEnd;
        }
        if (textEnd >= json.size()) break;
        std::string text = json.substr(textStart, textEnd - textStart);

        size_t next = json.find(textMarker, textEnd);
        std::string scope =
            json.substr(textEnd, (next == std::string::npos ? json.size() : next) - textEnd);

        std::string comment;
        const std::string annotationMarker = "\"annotation\":\"";
        size_t annotationPos = scope.find(annotationMarker);
        if (annotationPos != std::string::npos) {
            size_t annotationStart = annotationPos + annotationMarker.size();
            size_t annotationEnd = annotationStart;
            while (annotationEnd < scope.size()) {
                if (scope[annotationEnd] == '"' && scope[annotationEnd - 1] != '\\') break;
                ++annotationEnd;
            }
            if (annotationEnd <= scope.size()) {
                comment = scope.substr(annotationStart, annotationEnd - annotationStart);
            }
        } else if (scope.find("\"mandarin_only\":true") != std::string::npos) {
            comment = "[官]";
        }
        int consumed = 0;
        const std::string consumedMarker = "\"consumed_bytes\":";
        size_t consumedPos = scope.find(consumedMarker);
        if (consumedPos != std::string::npos) {
            size_t valStart = consumedPos + consumedMarker.size();
            size_t valEnd = valStart;
            while (valEnd < scope.size() && scope[valEnd] >= '0' && scope[valEnd] <= '9') ++valEnd;
            if (valEnd > valStart) consumed = std::stoi(scope.substr(valStart, valEnd - valStart));
        }
        std::string reading = extractStringField(scope, "\"reading\":\"");
        std::string mandarinReading = extractStringField(scope, "\"mandarin_reading\":\"");
        out.push_back(RankedItem{std::move(text), std::move(comment), consumed, std::move(reading), std::move(mandarinReading)});
        cursor = textEnd;
    }
    return out;
}

class GannyuEngineState : public fcitx::InputContextProperty {
public:
    std::string buffer;
    bool sentenceMode = false;
    std::vector<std::vector<RankedItem>> sentenceSegments;
    int currentSegment = 0;
    std::string accumulatedText;
    std::string accumulatedReading;
    std::string accumulatedMandarinReading;
    std::string originalInput;
};

class GannyuEngine;

class GannyuRegionAction : public fcitx::Action {
public:
    explicit GannyuRegionAction(GannyuEngine *engine) : engine_(engine) {}

    std::string shortText(fcitx::InputContext *ic) const override;
    std::string icon(fcitx::InputContext *ic) const override;

private:
    GannyuEngine *engine_;
};

class GannyuCandidateWord : public fcitx::CandidateWord {
public:
    GannyuCandidateWord(const RankedItem &item, GannyuEngine *engine, GannyuEngineState *state)
        : text_(item.text), reading_(item.reading), mandarinReading_(item.mandarinReading),
          engine_(engine), state_(state), consumed_(item.consumedBytes) {
        setText(fcitx::Text(buildCandidateDisplay(item)));
    }
    void select(fcitx::InputContext *ic) const override;
private:
    std::string text_;
    std::string reading_;
    std::string mandarinReading_;
    GannyuEngine *engine_;
    GannyuEngineState *state_;
    int consumed_;
};

class GannyuRegionCandidateWord : public fcitx::CandidateWord {
public:
    GannyuRegionCandidateWord(std::string label, std::function<void()> onSelect)
        : onSelect_(std::move(onSelect)) {
        setText(fcitx::Text(std::move(label)));
    }
    void select(fcitx::InputContext *ic) const override {
        if (onSelect_) onSelect_();
        ic->inputPanel().reset();
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
private:
    std::function<void()> onSelect_;
};

class GannyuEngine : public fcitx::InputMethodEngineV2 {
public:
    GannyuEngine(fcitx::Instance *instance)
        : instance_(instance),
          factory_([](fcitx::InputContext &) { return new GannyuEngineState; }) {
        instance_->inputContextManager().registerProperty("gannyuState", &factory_);
        regionAction_ = std::make_unique<GannyuRegionAction>(this);
        instance_->userInterfaceManager().registerAction("gannyu-region", regionAction_.get());
        regionAction_->setMenu(&regionMenu_);
        manifestPath_ = resolveManifestPath();
        regionId_ = std::getenv("GANNYU_REGION_ID") ? std::getenv("GANNYU_REGION_ID") : "lancong";
        loadRegions();
        ensurePipeline();
    }

    ~GannyuEngine() override {
        if (pipeline_) gannyu_pipeline_destroy(pipeline_);
    }

    void activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &) override {}

    std::string currentRegionLabel() const {
        for (size_t i = 0; i < regionIds_.size(); ++i) {
            if (regionIds_[i] == regionId_) {
                return i < regionLabels_.size() ? regionLabels_[i] : regionIds_[i];
            }
        }
        if (!regionLabels_.empty()) {
            return regionLabels_.front();
        }
        if (!regionIds_.empty()) {
            return regionIds_.front();
        }
        return "赣";
    }

    void handleCandidateSelect(fcitx::InputContext *ic, GannyuEngineState *state,
                               const std::string &text, const std::string &reading,
                               const std::string &mandarinReading, int consumed) {
        if (pipeline_) gannyu_pipeline_user_dict_boost(pipeline_, text.c_str(), nullptr);
        state->accumulatedText += text;
        if (!reading.empty()) {
            if (!state->accumulatedReading.empty()) state->accumulatedReading += " ";
            state->accumulatedReading += reading;
        }
        if (!mandarinReading.empty()) {
            if (!state->accumulatedMandarinReading.empty()) state->accumulatedMandarinReading += " ";
            state->accumulatedMandarinReading += mandarinReading;
        }
        if (state->buffer.size() >= 4 && consumed > 0 && consumed < (int)state->buffer.size()) {
            state->buffer = state->buffer.substr(consumed);
            lastCandidates_.clear();
            refresh(ic, state);
            return;
        }
        trySaveUserWord(state);
        resetState(ic, state);
    }

    void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override {
        if (event.isRelease()) return;
        auto *ic = event.inputContext();
        auto *state = ic->propertyFor(&factory_);
        auto key = event.key();
        auto sym = key.sym();

        if (sym == FcitxKey_Control_L || sym == FcitxKey_Control_R ||
            sym == FcitxKey_Alt_L || sym == FcitxKey_Alt_R) return;
        if (key.states().testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt, fcitx::KeyState::Super})) return;

        ensureRuntimeReady();

        if (sym >= FcitxKey_1 && sym <= FcitxKey_9) {
            /* Pass through number keys when no candidates are showing */
            if (lastCandidates_.empty() && state->buffer.empty()) {
                auto cl = ic->inputPanel().candidateList();
                if (!cl || cl->size() == 0) return;
            }
            if (state->sentenceMode) {
                size_t idx = sym - FcitxKey_1;
                if (idx < lastCandidates_.size()) {
                    ic->commitString(lastCandidates_[idx].text);
                    handleCandidateSelect(ic, state, lastCandidates_[idx].text,
                        lastCandidates_[idx].reading, lastCandidates_[idx].mandarinReading,
                        lastCandidates_[idx].consumedBytes);
                }
                state->currentSegment++;
                if (state->currentSegment >= (int)state->sentenceSegments.size()) {
                    resetState(ic, state);
                } else {
                    refreshSentenceSegment(ic, state);
                }
                event.filterAndAccept();
                return;
            }
            {
                size_t localIdx = sym - FcitxKey_1;
                int currentPage = 0;
                auto cl = ic->inputPanel().candidateList();
                if (cl && cl->toPageable()) currentPage = cl->toPageable()->currentPage();
                size_t globalIdx = (size_t)(currentPage * 9) + localIdx;
                if (globalIdx < lastCandidates_.size()) {
                    ic->commitString(lastCandidates_[globalIdx].text);
                    handleCandidateSelect(ic, state, lastCandidates_[globalIdx].text,
                        lastCandidates_[globalIdx].reading, lastCandidates_[globalIdx].mandarinReading,
                        lastCandidates_[globalIdx].consumedBytes);
                }
                event.filterAndAccept();
                return;
            }
        }
        if (sym == FcitxKey_space) {
            if (!lastCandidates_.empty()) {
                int currentPage = 0;
                auto cl = ic->inputPanel().candidateList();
                if (cl && cl->toPageable()) currentPage = cl->toPageable()->currentPage();
                size_t firstIdx = (size_t)(currentPage * 9);
                if (firstIdx >= lastCandidates_.size()) firstIdx = 0;
                ic->commitString(lastCandidates_[firstIdx].text);
                handleCandidateSelect(ic, state, lastCandidates_[firstIdx].text,
                    lastCandidates_[firstIdx].reading, lastCandidates_[firstIdx].mandarinReading,
                    lastCandidates_[firstIdx].consumedBytes);
                event.filterAndAccept();
                return;
            }
            if (!state->buffer.empty()) {
                resetState(ic, state);
                event.filterAndAccept();
                return;
            }
            /* Pass through space when buffer is empty and no candidates are showing */
            {
                auto cl = ic->inputPanel().candidateList();
                if (!cl || cl->size() == 0) return;
            }
            event.filterAndAccept();
            return;
        }
        if (sym == FcitxKey_Return) {
            if (!state->buffer.empty()) {
                ic->commitString(state->buffer);
                state->accumulatedText.clear();
                state->accumulatedReading.clear();
                state->accumulatedMandarinReading.clear();
                state->originalInput.clear();
                resetState(ic, state);
                event.filterAndAccept();
            }
            return;
        }
        if (sym == FcitxKey_BackSpace) {
            if (!state->buffer.empty()) {
                state->buffer.pop_back();
                refresh(ic, state);
                event.filterAndAccept();
            }
            return;
        }
        if (sym == FcitxKey_period && !lastCandidates_.empty()) {
            auto cl = ic->inputPanel().candidateList();
            if (cl && cl->toPageable() && cl->toPageable()->hasNext()) {
                cl->toPageable()->next();
                int newPage = cl->toPageable()->currentPage();
                size_t firstOnPage = (size_t)(newPage * 9);
                int consumed = (firstOnPage < lastCandidates_.size())
                    ? lastCandidates_[firstOnPage].consumedBytes : 0;
                std::string display = buildPreeditDisplay(state->buffer, consumed);
                fcitx::Text preedit(display, fcitx::TextFormatFlag::Underline);
                preedit.setCursor(preeditCursorPosition(state->buffer, display));
                auto &panel = ic->inputPanel();
                panel.setClientPreedit(preedit);
                ic->updatePreedit();
                ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
            }
            event.filterAndAccept();
            return;
        }
        if (sym == FcitxKey_comma && !lastCandidates_.empty()) {
            auto cl = ic->inputPanel().candidateList();
            if (cl && cl->toPageable() && cl->toPageable()->hasPrev()) {
                cl->toPageable()->prev();
                int newPage = cl->toPageable()->currentPage();
                size_t firstOnPage = (size_t)(newPage * 9);
                int consumed = (firstOnPage < lastCandidates_.size())
                    ? lastCandidates_[firstOnPage].consumedBytes : 0;
                std::string display = buildPreeditDisplay(state->buffer, consumed);
                fcitx::Text preedit(display, fcitx::TextFormatFlag::Underline);
                preedit.setCursor(preeditCursorPosition(state->buffer, display));
                auto &panel = ic->inputPanel();
                panel.setClientPreedit(preedit);
                ic->updatePreedit();
                ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
            }
            event.filterAndAccept();
            return;
        }
        if (sym == FcitxKey_Escape) {
            if (!state->buffer.empty()) {
                state->accumulatedText.clear();
                state->accumulatedReading.clear();
                state->accumulatedMandarinReading.clear();
                state->originalInput.clear();
                resetState(ic, state);
                event.filterAndAccept();
            }
            return;
        }
        if (sym >= FcitxKey_a && sym <= FcitxKey_z) {
            if (state->buffer.empty()) {
                state->accumulatedText.clear();
                state->accumulatedReading.clear();
                state->accumulatedMandarinReading.clear();
            }
            state->buffer.push_back(static_cast<char>(sym));
            state->originalInput = state->buffer;
            refresh(ic, state);
            event.filterAndAccept();
            return;
        }
        if (sym >= FcitxKey_A && sym <= FcitxKey_Z) {
            if (state->buffer.empty()) {
                state->accumulatedText.clear();
                state->accumulatedReading.clear();
                state->accumulatedMandarinReading.clear();
            }
            state->buffer.push_back(static_cast<char>(sym - FcitxKey_A + FcitxKey_a));
            state->originalInput = state->buffer;
            refresh(ic, state);
            event.filterAndAccept();
            return;
        }
        if (sym == FcitxKey_apostrophe) {
            if (state->buffer.empty()) {
                state->accumulatedText.clear();
                state->accumulatedReading.clear();
                state->accumulatedMandarinReading.clear();
            }
            state->buffer.push_back('\'');
            state->originalInput = state->buffer;
            refresh(ic, state);
            event.filterAndAccept();
            return;
        }
        /* When there is a composing buffer and the user types a symbol
         * (e.g. '='), commit the first candidate first, then pass the symbol
         * through so it is not swallowed. If no candidate exists, commit the
         * buffered letters (excluding apostrophes) instead. */
        if (!state->buffer.empty() && sym >= 0x20 && sym <= 0x7E) {
            if (!lastCandidates_.empty()) {
                int currentPage = 0;
                auto cl = ic->inputPanel().candidateList();
                if (cl && cl->toPageable()) currentPage = cl->toPageable()->currentPage();
                size_t firstIdx = (size_t)(currentPage * 9);
                if (firstIdx >= lastCandidates_.size()) firstIdx = 0;
                ic->commitString(lastCandidates_[firstIdx].text);
                handleCandidateSelect(ic, state, lastCandidates_[firstIdx].text,
                    lastCandidates_[firstIdx].reading, lastCandidates_[firstIdx].mandarinReading,
                    lastCandidates_[firstIdx].consumedBytes);
            } else {
                /* No candidate: commit the buffered letters (drop apostrophes). */
                std::string letters;
                for (char ch : state->buffer) {
                    if (ch != '\'') letters.push_back(ch);
                }
                ic->commitString(letters);
                resetState(ic, state);
            }
            std::string fw = chineseFullwidthSymbol(static_cast<int>(sym));
            ic->commitString(fw.empty() ? std::string(1, static_cast<char>(sym)) : fw);
            event.filterAndAccept();
            return;
        }
        /* Convert common ASCII punctuation to Chinese full-width when buffer is empty */
        if (state->buffer.empty() && sym >= 0x20 && sym <= 0x7E) {
            std::string fw = chineseFullwidthSymbol(static_cast<int>(sym));
            if (!fw.empty()) {
                ic->commitString(fw);
                event.filterAndAccept();
                return;
            }
        }
    }

private:
    void ensureRuntimeReady() {
        // Auto-started fcitx5 can beat a late-mounted resource directory after
        // reboot, so retry region/pipeline loading when the user actually types.
        if (regionIds_.empty()) loadRegions();
        ensurePipeline();
    }

    void parseSegmentSentence(const std::string &json, GannyuEngineState *state) {
        state->sentenceSegments.clear();
        size_t cursor = json.find('[');
        if (cursor == std::string::npos) return;
        cursor++;
        while (cursor < json.size()) {
            if (json[cursor] == ']') break;
            if (json[cursor] == '[') {
                cursor++;
                int depth = 1;
                size_t start = cursor;
                while (cursor < json.size() && depth > 0) {
                    if (json[cursor] == '[') depth++;
                    else if (json[cursor] == ']') depth--;
                    cursor++;
                }
                std::string inner = json.substr(start, cursor - start - 1);
                state->sentenceSegments.push_back(parseRankedItems(inner));
            } else {
                cursor++;
            }
        }
    }

    void ensurePipeline() {
        if (pipeline_) return;
        GannyuPipelineHandle *h = nullptr;
        const char *region = regionId_.empty() ? nullptr : regionId_.c_str();
        int code = gannyu_pipeline_create(manifestPath_.c_str(), region, &h);
        if (code == 0) pipeline_ = h;
    }

    void loadRegions() {
        char *json = nullptr;
        if (gannyu_region_list(manifestPath_.c_str(), &json) == 0 && json) {
            regionIds_ = parseStringField(json, "id");
            regionLabels_ = parseStringField(json, "name_zh");
            gannyu_string_destroy(json);
        }
        if (regionId_.empty() && !regionIds_.empty()) {
            regionId_ = regionIds_.front();
        }
        buildRegionMenu();
    }

    void switchRegion(const std::string &id) {
        if (id.empty() || id == regionId_) {
            return;
        }
        if (pipeline_) {
            gannyu_pipeline_destroy(pipeline_);
            pipeline_ = nullptr;
        }
        regionId_ = id;
        ensurePipeline();
        syncRegionActionChecks();
    }

    void showRegionSwitcher(fcitx::InputContext *ic) {
        auto &panel = ic->inputPanel();
        panel.reset();
        panel.setAuxUp(fcitx::Text("切换地区"));
        auto list = std::make_unique<fcitx::CommonCandidateList>();
        list->setLayoutHint(fcitx::CandidateLayoutHint::Vertical);
        list->setPageSize(9);
        list->setSelectionKey(selectionKeys());
        for (size_t i = 0; i < regionIds_.size(); ++i) {
            std::string label = i < regionLabels_.size() ? regionLabels_[i] : regionIds_[i];
            if (regionIds_[i] == regionId_) label += " ✓";
            std::string id = regionIds_[i];
            list->append<GannyuRegionCandidateWord>(label, [this, id]() { switchRegion(id); });
        }
        if (!regionIds_.empty()) list->setGlobalCursorIndex(0);
        panel.setCandidateList(std::move(list));
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }

    void buildRegionMenu() {
        for (auto *action : regionMenu_.actions()) {
            regionMenu_.removeAction(action);
        }
        regionMenuActions_.clear();
        for (size_t i = 0; i < regionIds_.size(); ++i) {
            const std::string label =
                i < regionLabels_.size() ? regionLabels_[i] : regionIds_[i];
            const std::string id = regionIds_[i];
            regionMenuActions_.emplace_back();
            auto &action = regionMenuActions_.back();
            action.setShortText(label);
            action.setCheckable(true);
            action.setChecked(id == regionId_);
            instance_->userInterfaceManager().registerAction(&action);
            action.connect<fcitx::SimpleAction::Activated>(
                [this, id](fcitx::InputContext *ic) {
                    switchRegion(id);
                    if (ic) {
                        if (auto *state = ic->propertyFor(&factory_)) {
                            resetState(ic, state);
                        }
                        if (regionAction_) {
                            regionAction_->update(ic);
                        }
                    }
                });
            regionMenu_.addAction(&action);
        }
    }

    void syncRegionActionChecks() {
        size_t index = 0;
        for (auto &action : regionMenuActions_) {
            action.setChecked(index < regionIds_.size() && regionIds_[index] == regionId_);
            ++index;
        }
    }

    // Use the shared FFI formatter so every native frontend follows the same
    // Fcitx5-compatible auto/manual separator rule without changing buffer.
    std::string buildPreeditDisplay(const std::string &buf, int consumed) {
        if (!pipeline_) return buf;
        char *formatted = nullptr;
        const size_t consumed_bytes = static_cast<size_t>(std::max(consumed, 0));
        if (gannyu_pipeline_format_preedit(
                pipeline_, buf.c_str(), consumed_bytes, &formatted) != 0 || !formatted) {
            return buf;
        }
        std::string display(formatted);
        gannyu_string_destroy(formatted);
        return display;
    }

    // Map the buffer length to a cursor position in the display string. The
    // display may contain visual separators (spaces) that are not part of the
    // buffer, so the cursor must skip them to sit at the end of the real input.
    int preeditCursorPosition(const std::string &buf, const std::string &display) {
        int non_sep = 0;
        for (int i = 0; i < (int)display.size(); ++i) {
            if (display[i] == ' ' || display[i] == '\'') continue;
            non_sep++;
            if (non_sep >= (int)buf.size()) return i + 1;
        }
        return (int)display.size();
    }

    void refresh(fcitx::InputContext *ic, GannyuEngineState *state) {
        ensureRuntimeReady();
        auto &panel = ic->inputPanel();
        // Clear the candidate list but keep the preedit set below. Avoid
        // panel.reset() which clears the preedit and can make fcitx5/apps
        // spuriously commit the composing text while typing fast.
        panel.setCandidateList(nullptr);
        if (state->buffer.empty()) {
            // Clear the preedit explicitly (setCandidateList(nullptr) does not
            // clear it), so a leftover composing text does not linger after
            // backspace empties the buffer.
            panel.setClientPreedit(fcitx::Text());
            ic->updatePreedit();
            ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
            lastCandidates_.clear();
            return;
        }

        lastCandidates_.clear();
        state->sentenceMode = false;
        state->sentenceSegments.clear();
        state->currentSegment = 0;

        std::vector<RankedItem> ranked;
        if (pipeline_) {
            if (state->sentenceMode && state->currentSegment < (int)state->sentenceSegments.size()) {
                ranked = state->sentenceSegments[state->currentSegment];
            } else {
                char *json = nullptr;
                int code = gannyu_pipeline_retrieve(pipeline_, state->buffer.c_str(), &json);
                if (code == 0 && json) {
                    ranked = parseRankedItems(json);
                    gannyu_string_destroy(json);
                }
            }
        }

        // Set preedit: if the first candidate tells us exactly how much of the buffer
        // it would consume, use that to show the active segment visually; otherwise
        // fall back to auto-segmentation boundaries.
        int firstConsumed = ranked.empty() ? 0 : ranked[0].consumedBytes;
        std::string display = buildPreeditDisplay(state->buffer, firstConsumed);
        fcitx::Text preedit(display, fcitx::TextFormatFlag::Underline);
        // Cursor tracks the actual buffer length, not the display length, so
        // fcitx5/apps do not see a cursor beyond the real input and spuriously
        // commit part of the preedit.
        preedit.setCursor(preeditCursorPosition(state->buffer, display));
        panel.setClientPreedit(preedit);

        if (!ranked.empty()) {
            auto list = std::make_unique<fcitx::CommonCandidateList>();
            list->setLayoutHint(fcitx::CandidateLayoutHint::Vertical);
            list->setPageSize(9);
            list->setSelectionKey(selectionKeys());
            for (auto &item : ranked) {
                lastCandidates_.push_back(item);
                list->append<GannyuCandidateWord>(item, this, state);
            }
            list->setGlobalCursorIndex(0);
            panel.setCandidateList(std::move(list));
        }
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }

    void refreshSentenceSegment(fcitx::InputContext *ic, GannyuEngineState *state) {
        auto &panel = ic->inputPanel();
        panel.reset();
        int seg = state->currentSegment;
        if (seg < 0 || seg >= (int)state->sentenceSegments.size()) {
            resetState(ic, state);
            return;
        }
        std::string preedit = "[" + std::to_string(seg + 1) + "/" +
                              std::to_string(state->sentenceSegments.size()) + "]";
        panel.setAuxUp(fcitx::Text(preedit));
        std::string segmented = buildPreeditDisplay(state->buffer, 0);
        panel.setClientPreedit(fcitx::Text(segmented, fcitx::TextFormatFlag::Underline));

        lastCandidates_.clear();
        auto &ranked = state->sentenceSegments[seg];
        if (!ranked.empty()) {
            auto list = std::make_unique<fcitx::CommonCandidateList>();
            list->setLayoutHint(fcitx::CandidateLayoutHint::Vertical);
            list->setPageSize(9);
            list->setSelectionKey(selectionKeys());
            for (auto &item : ranked) {
                lastCandidates_.push_back(item);
                list->append<GannyuCandidateWord>(item, this, state);
            }
            list->setGlobalCursorIndex(0);
            panel.setCandidateList(std::move(list));
        }
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }

    void resetState(fcitx::InputContext *ic, GannyuEngineState *state) {
        state->buffer.clear();
        state->sentenceMode = false;
        state->sentenceSegments.clear();
        state->currentSegment = 0;
        lastCandidates_.clear();
        ic->inputPanel().reset();
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }

    void trySaveUserWord(GannyuEngineState *state) {
        int count = 0;
        for (size_t i = 0; i < state->accumulatedText.size(); ) {
            unsigned char lead = (unsigned char)state->accumulatedText[i];
            i += (lead < 0x80) ? 1 : ((lead < 0xE0) ? 2 : ((lead < 0xF0) ? 3 : 4));
            ++count;
        }
        if (count >= 2 && !state->accumulatedReading.empty() && pipeline_) {
            gannyu_pipeline_user_dict_add(
                pipeline_,
                state->accumulatedText.c_str(),
                state->accumulatedReading.c_str(),
                state->accumulatedMandarinReading.c_str(),
                nullptr);
        }
        state->accumulatedText.clear();
        state->accumulatedReading.clear();
        state->accumulatedMandarinReading.clear();
        state->originalInput.clear();
    }

    fcitx::Instance *instance_;
    fcitx::FactoryFor<GannyuEngineState> factory_;
    GannyuPipelineHandle *pipeline_ = nullptr;
    std::string manifestPath_;
    std::string regionId_;
    std::vector<std::string> regionIds_;
    std::vector<std::string> regionLabels_;
    std::unique_ptr<GannyuRegionAction> regionAction_;
    fcitx::Menu regionMenu_;
    std::list<fcitx::SimpleAction> regionMenuActions_;
    bool regionMode_ = false;
    std::vector<RankedItem> lastCandidates_;
};

void GannyuCandidateWord::select(fcitx::InputContext *ic) const {
    ic->commitString(text_);
    if (engine_) engine_->handleCandidateSelect(ic, state_, text_, reading_, mandarinReading_, consumed_);
}

std::string GannyuRegionAction::shortText(fcitx::InputContext *) const {
    return engine_ ? engine_->currentRegionLabel() : "赣";
}

std::string GannyuRegionAction::icon(fcitx::InputContext *) const { return ""; }

class GannyuEngineFactory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        return new GannyuEngine(manager->instance());
    }
};

} // namespace

FCITX_ADDON_FACTORY(GannyuEngineFactory)
