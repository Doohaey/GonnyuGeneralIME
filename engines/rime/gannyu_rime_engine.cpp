#include "gannyu_input.h"

#include <rime_api.h>

#include <algorithm>
#include <array>
#include <cctype>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <filesystem>
#include <memory>
#include <limits>
#include <mutex>
#include <optional>
#include <sstream>
#include <string>
#include <vector>

struct GannyuPipelineHandle {
  RimeSessionId session = 0;
  std::string schema_id;
  std::string region_id;
  bool ipa_scheme = false;
  size_t candidate_limit = 0;
  std::array<int, 6> input_options{{1, 1, 1, 1, 1, 0}};
  bool synonyms_enabled = true;
  bool wenbai_enabled = true;
};

namespace {

std::string SchemaFor(const GannyuPipelineHandle& handle) {
  return "gannyu_" + handle.region_id + (handle.ipa_scheme ? "_ipa" : "");
}

constexpr int kOk = 0;
constexpr int kInvalidArgument = 1;
constexpr int kLoadFailure = 2;
constexpr int kSerializeFailure = 3;
constexpr int kBackspace = 0xff08;
constexpr int kReturn = 0xff0d;
constexpr int kLeft = 0xff51;
constexpr int kRight = 0xff53;
constexpr int kPageUp = 0xff55;
constexpr int kPageDown = 0xff56;
constexpr int kDelete = 0xffff;

thread_local std::string g_last_error;

void SetError(std::string message) { g_last_error = std::move(message); }

template <typename Function>
int AbiStatus(Function&& function) noexcept {
  try {
    return function();
  } catch (const std::exception& error) {
    try {
      SetError(std::string("native Rime exception: ") + error.what());
    } catch (...) {
    }
  } catch (...) {
    try {
      SetError("unknown native Rime exception");
    } catch (...) {
    }
  }
  return kLoadFailure;
}

char* CopyOut(const std::string& value) {
  auto* output = static_cast<char*>(std::malloc(value.size() + 1));
  if (output == nullptr) {
    return nullptr;
  }
  std::memcpy(output, value.c_str(), value.size() + 1);
  return output;
}

std::string JsonEscape(const char* value) {
  std::string output;
  for (const unsigned char c : std::string(value ? value : "")) {
    switch (c) {
      case '\\': output += "\\\\"; break;
      case '"': output += "\\\""; break;
      case '\n': output += "\\n"; break;
      case '\r': output += "\\r"; break;
      case '\t': output += "\\t"; break;
      default:
        if (c < 0x20) {
          char encoded[7] = {};
          std::snprintf(encoded, sizeof(encoded), "\\u%04x", c);
          output += encoded;
        } else {
          output += static_cast<char>(c);
        }
    }
  }
  return output;
}

std::string JsonEscape(const std::string& value) { return JsonEscape(value.c_str()); }

size_t Utf8CountBefore(const char* text, size_t bytes) {
  size_t characters = 0;
  for (size_t index = 0; text != nullptr && index < bytes && text[index] != '\0'; ++index) {
    if ((static_cast<unsigned char>(text[index]) & 0xc0) != 0x80) {
      ++characters;
    }
  }
  return characters;
}

bool IsCompositionText(const std::string& text) {
  return !text.empty() && std::all_of(text.begin(), text.end(), [](unsigned char c) {
    return std::isalpha(c) || c == '\'';
  });
}

std::optional<std::string> JsonStringField(const char* json, const char* field) {
  if (json == nullptr) return std::nullopt;
  const std::string needle = std::string("\"") + field + "\"";
  const char* cursor = std::strstr(json, needle.c_str());
  if (cursor == nullptr) return std::nullopt;
  cursor = std::strchr(cursor + needle.size(), ':');
  if (cursor == nullptr) return std::nullopt;
  while (*++cursor != '\0' && std::isspace(static_cast<unsigned char>(*cursor))) {}
  if (*cursor != '"') return std::nullopt;
  ++cursor;
  std::string output;
  while (*cursor != '\0') {
    const char current = *cursor++;
    if (current == '"') return output;
    if (current != '\\') {
      output += current;
      continue;
    }
    const char escaped = *cursor++;
    switch (escaped) {
      case '"': output += '"'; break;
      case '\\': output += '\\'; break;
      case '/': output += '/'; break;
      case 'b': output += '\b'; break;
      case 'f': output += '\f'; break;
      case 'n': output += '\n'; break;
      case 'r': output += '\r'; break;
      case 't': output += '\t'; break;
      default: return std::nullopt;  // Mobile callers send UTF-8, not \\u escapes.
    }
  }
  return std::nullopt;
}

struct Runtime {
  std::mutex mutex;
  bool setup = false;
  bool initialized = false;
  std::string shared_data_dir;
  std::string user_data_dir;
  std::string prebuilt_data_dir;
  std::string staging_dir;
  RimeApi* api = nullptr;
  // Non-owning: callers remain the sole owners of pipeline handles.
  std::vector<GannyuPipelineHandle*> handles;
};

Runtime& GlobalRuntime() {
  static Runtime runtime;
  return runtime;
}

bool HasRegisteredHandleLocked(Runtime& runtime, GannyuPipelineHandle* handle) {
  if (std::find(runtime.handles.begin(), runtime.handles.end(), handle) != runtime.handles.end()) {
    return true;
  }
  SetError("engine handle is unavailable");
  return false;
}

void FinalizeUnusedRuntimeLocked(Runtime& runtime) {
  if (runtime.handles.empty() && runtime.initialized && runtime.api != nullptr) {
    runtime.api->finalize();
    runtime.initialized = false;
  }
}

bool EnsureRuntimeLocked(const char* shared_data_dir, const char* prebuilt_data_dir, const char* user_data_dir) {
  if (shared_data_dir == nullptr || *shared_data_dir == '\0' ||
      prebuilt_data_dir == nullptr || *prebuilt_data_dir == '\0' ||
      user_data_dir == nullptr || *user_data_dir == '\0') {
    SetError("Rime requires explicit shared, prebuilt, and user data directories");
    return false;
  }
  Runtime& runtime = GlobalRuntime();
  if (runtime.setup) {
    if (runtime.shared_data_dir != shared_data_dir ||
        runtime.prebuilt_data_dir != prebuilt_data_dir ||
        runtime.user_data_dir != user_data_dir) {
      SetError("librime is process-global; all sessions must use the same data directories");
      return false;
    }
  } else {
    runtime.shared_data_dir = shared_data_dir;
    runtime.user_data_dir = user_data_dir;
    runtime.prebuilt_data_dir = prebuilt_data_dir;
    runtime.staging_dir = runtime.user_data_dir + "/build";
    runtime.api = rime_get_api();
    if (runtime.api == nullptr) {
      SetError("rime_get_api returned null");
      return false;
    }
    RimeTraits traits{};
    RIME_STRUCT_INIT(RimeTraits, traits);
    traits.shared_data_dir = runtime.shared_data_dir.c_str();
    traits.user_data_dir = runtime.user_data_dir.c_str();
    traits.prebuilt_data_dir = runtime.prebuilt_data_dir.c_str();
    traits.staging_dir = runtime.staging_dir.c_str();
    traits.app_name = "rime.gonnyu.mobile";
    traits.log_dir = "";
    runtime.api->setup(&traits);
    runtime.setup = true;
  }
  if (!runtime.initialized) {
    runtime.api->initialize(nullptr);
    runtime.initialized = true;
  }
  return true;
}

RimeApi* Api() { return GlobalRuntime().api; }

void ApplyInputOptions(GannyuPipelineHandle* handle) {
  Api()->set_option(handle->session, "gannyu_batch", True);
  const char* names[] = {"gannyu_no_annotations", "gannyu_no_relations", "gannyu_no_fuzzy",
                         "gannyu_no_mandarin", "gannyu_no_fuzzy_rules"};
  for (size_t index = 0; index < 5; ++index) {
    Api()->set_option(handle->session, names[index], handle->input_options[index] ? False : True);
  }
  Api()->set_option(handle->session, "gannyu_both", handle->input_options[5] == 0 ? True : False);
  Api()->set_option(handle->session, "gannyu_new", handle->input_options[5] == 1 ? True : False);
  Api()->set_option(handle->session, "gannyu_old", handle->input_options[5] == 2 ? True : False);
  Api()->set_option(handle->session, "gannyu_no_synonyms", handle->synonyms_enabled ? False : True);
  Api()->set_option(handle->session, "gannyu_no_wenbai", handle->wenbai_enabled ? False : True);
  Api()->set_option(handle->session, "gannyu_batch", False);
}

bool StartSessionLocked(GannyuPipelineHandle* handle) {
  handle->session = Api()->create_session();
  if (handle->session == 0) {
    SetError("failed to create Rime session");
    return false;
  }
  if (!Api()->select_schema(handle->session, handle->schema_id.c_str())) {
    Api()->destroy_session(handle->session);
    handle->session = 0;
    SetError("failed to select Rime schema: " + handle->schema_id);
    return false;
  }
  ApplyInputOptions(handle);
  return true;
}

bool StopSessionLocked(GannyuPipelineHandle* handle) {
  if (handle->session == 0) return true;
  if (!Api()->destroy_session(handle->session)) {
    SetError("failed to destroy Rime session");
    return false;
  }
  handle->session = 0;
  return true;
}

std::optional<std::string> TakeCommit(RimeSessionId session) {
  RimeCommit commit{};
  RIME_STRUCT_INIT(RimeCommit, commit);
  if (!Api()->get_commit(session, &commit)) return std::nullopt;
  auto free_commit = [](RimeCommit* value) { Api()->free_commit(value); };
  std::unique_ptr<RimeCommit, decltype(free_commit)> commit_guard(&commit, free_commit);
  return std::string(commit.text ? commit.text : "");
}

std::string Snapshot(GannyuPipelineHandle* handle, bool handled, const std::optional<std::string>& commit) {
  RimeContext context{};
  RIME_STRUCT_INIT(RimeContext, context);
  RimeStatus status{};
  RIME_STRUCT_INIT(RimeStatus, status);
  bool has_context = Api()->get_context(handle->session, &context);
  auto free_context = [&has_context](RimeContext* value) {
    if (has_context) Api()->free_context(value);
  };
  std::unique_ptr<RimeContext, decltype(free_context)> context_guard(&context, free_context);
  const bool has_status = Api()->get_status(handle->session, &status);
  auto free_status = [has_status](RimeStatus* value) {
    if (has_status) Api()->free_status(value);
  };
  std::unique_ptr<RimeStatus, decltype(free_status)> status_guard(&status, free_status);
  std::ostringstream output;
  const char* raw_input = Api()->get_input(handle->session);
  output << "{\"handled\":" << (handled ? "true" : "false") << ",\"rawInput\":\"";
  const char* preedit = has_context && context.composition.preedit ? context.composition.preedit : "";
  output << JsonEscape(raw_input) << "\",\"preedit\":\"" << JsonEscape(preedit) << "\",\"caret\":";
  const char* caret_text = has_context ? preedit : raw_input;
  const size_t caret_bytes = has_context
      ? static_cast<size_t>(std::max(context.composition.cursor_pos, 0))
      : Api()->get_caret_pos(handle->session);
  output << Utf8CountBefore(caret_text, caret_bytes);
  if (commit.has_value() && !commit->empty()) output << ",\"commitText\":\"" << JsonEscape(*commit) << "\"";
  output << ",\"candidates\":[";
  bool first_candidate = true;
  auto append_candidate = [&](const RimeCandidate& candidate, int index) {
    if (!first_candidate) output << ',';
    first_candidate = false;
    output << "{\"text\":\"" << JsonEscape(candidate.text) << "\"";
    if (candidate.comment != nullptr && candidate.comment[0] != '\0') output << ",\"annotation\":\"" << JsonEscape(candidate.comment) << "\"";
    output << ",\"globalIndex\":" << index;
    output << ",\"pageIndex\":" << index << ",\"deletable\":false}";
  };
  auto append_context_candidates = [&] {
    if (!has_context) return;
    for (int index = 0; index < context.menu.num_candidates &&
         (handle->candidate_limit == 0 || index + context.menu.page_no * context.menu.page_size < static_cast<int>(handle->candidate_limit)); ++index) {
      append_candidate(context.menu.candidates[index], context.menu.page_no * context.menu.page_size + index);
    }
  };
  bool iterated_candidates = false;
  if (handle->candidate_limit > 0 && has_context && context.menu.page_size > 0 &&
      RIME_PROVIDED(Api(), candidate_list_from_index) &&
      RIME_PROVIDED(Api(), candidate_list_next) && RIME_PROVIDED(Api(), candidate_list_end)) {
    RimeCandidateListIterator iterator{};
    const int start_index = context.menu.page_no * context.menu.page_size;
    if (Api()->candidate_list_from_index(handle->session, &iterator, start_index)) {
      auto end_iteration = [](RimeCandidateListIterator* value) { Api()->candidate_list_end(value); };
      std::unique_ptr<RimeCandidateListIterator, decltype(end_iteration)> guard(&iterator, end_iteration);
      iterated_candidates = true;
      while (iterator.index + 1 < static_cast<int>(handle->candidate_limit) &&
             Api()->candidate_list_next(&iterator)) {
        append_candidate(iterator.candidate, iterator.index);
      }
    }
  }
  const int original_page = has_context ? context.menu.page_no : 0;
  if (!iterated_candidates) append_context_candidates();
  if (!iterated_candidates && handle->candidate_limit > 0 && has_context) {
    while (context.menu.page_size > 0 && !context.menu.is_last_page && context.menu.page_no * context.menu.page_size < static_cast<int>(handle->candidate_limit)) {
      const int previous_page = context.menu.page_no;
      has_context = false;
      Api()->free_context(&context);
      Api()->process_key(handle->session, kPageDown, 0);
      RIME_STRUCT_INIT(RimeContext, context);
      has_context = Api()->get_context(handle->session, &context);
      append_context_candidates();
      if (!has_context || context.menu.page_no <= previous_page) break;
    }
    while (has_context && context.menu.page_no > original_page) {
      has_context = false;
      Api()->free_context(&context);
      Api()->process_key(handle->session, kPageUp, 0);
      RIME_STRUCT_INIT(RimeContext, context);
      has_context = Api()->get_context(handle->session, &context);
    }
  }
  output << "]";
  if (has_context && context.menu.num_candidates > 0) {
    output << ",\"highlightedIndex\":" << context.menu.highlighted_candidate_index;
  }
  output << ",\"pageNumber\":" << (handle->candidate_limit > 0 ? 0 : (has_context ? context.menu.page_no : 0));
  output << ",\"hasPreviousPage\":" << (handle->candidate_limit > 0 ? "false" : (has_context && context.menu.page_no > 0 ? "true" : "false"));
  output << ",\"hasNextPage\":"
         << (handle->candidate_limit > 0 ? "false" : (has_context && context.menu.num_candidates > 0 && !context.menu.is_last_page ? "true" : "false"));
  output << ",\"schemaId\":\"" << JsonEscape(handle->schema_id) << "\"";
  output << ",\"asciiMode\":" << (has_status && status.is_ascii_mode ? "true" : "false") << '}';
  context_guard.reset();
  status_guard.reset();
  return output.str();
}

int WriteSnapshot(GannyuPipelineHandle* handle, bool handled, const std::optional<std::string>& commit, char** out_json) {
  if (out_json == nullptr) return kInvalidArgument;
  *out_json = CopyOut(Snapshot(handle, handled, commit));
  if (*out_json == nullptr) {
    SetError("failed to allocate JSON snapshot");
    return kSerializeFailure;
  }
  return kOk;
}

const char* ResolveSharedData(const char* argument) {
  if (argument != nullptr && *argument != '\0') return argument;
  return std::getenv("GANNYU_RIME_SHARED_DATA_DIR");
}

int Create(const char* shared_data_dir,
           const char* prebuilt_data_dir,
           const char* region_id,
           const char* user_data_dir,
           GannyuPipelineHandle** out_handle) {
  g_last_error.clear();
  if (out_handle == nullptr) return kInvalidArgument;
  *out_handle = nullptr;
  const char* shared = ResolveSharedData(shared_data_dir);
  Runtime& runtime = GlobalRuntime();
  std::lock_guard<std::mutex> runtime_lock(runtime.mutex);
  if (!EnsureRuntimeLocked(shared, prebuilt_data_dir, user_data_dir)) return kLoadFailure;
  auto handle = std::make_unique<GannyuPipelineHandle>();
  const std::string region = region_id && *region_id ? region_id : "lancong";
  handle->region_id = region;
  handle->schema_id = SchemaFor(*handle);
  if (!StartSessionLocked(handle.get())) {
    FinalizeUnusedRuntimeLocked(runtime);
    return kLoadFailure;
  }
  try {
    runtime.handles.push_back(handle.get());
  } catch (...) {
    StopSessionLocked(handle.get());
    FinalizeUnusedRuntimeLocked(runtime);
    throw;
  }
  *out_handle = handle.release();
  return kOk;
}

}  // namespace

extern "C" {

int gannyu_pipeline_create(const char* shared_data_dir, const char* region_id, GannyuPipelineHandle** out_handle) {
  return AbiStatus([&] {
    return Create(shared_data_dir,
                  std::getenv("GANNYU_RIME_PREBUILT_DATA_DIR"),
                  region_id,
                  std::getenv("GANNYU_RIME_USER_DATA_DIR"),
                  out_handle);
  });
}

int gannyu_pipeline_create_with_user_data_dir(const char* shared_data_dir, const char* region_id, const char* user_data_dir, GannyuPipelineHandle** out_handle) {
  return AbiStatus([&] {
    const char* prebuilt = std::getenv("GANNYU_RIME_PREBUILT_DATA_DIR");
    std::string fallback;
    if (prebuilt == nullptr || *prebuilt == '\0') {
      fallback = std::string(user_data_dir ? user_data_dir : "") + "/build";
      prebuilt = fallback.c_str();
    }
    return Create(shared_data_dir, prebuilt, region_id, user_data_dir, out_handle);
  });
}

int gannyu_engine_create(const GannyuEngineConfig* config, GannyuPipelineHandle** out_handle) {
  return AbiStatus([&] {
    if (config == nullptr || config->struct_size < sizeof(GannyuEngineConfig)) {
      SetError("engine config is missing or older than the required ABI");
      return kInvalidArgument;
    }
    return Create(config->shared_data_dir,
                  config->prebuilt_data_dir,
                  config->region_id,
                  config->user_data_dir,
                  out_handle);
  });
}

int gannyu_last_error(char** out_error) {
  return AbiStatus([&] {
    if (out_error == nullptr) return kInvalidArgument;
    *out_error = nullptr;
    if (g_last_error.empty()) return kOk;
    *out_error = CopyOut(g_last_error);
    g_last_error.clear();
    return *out_error == nullptr ? kSerializeFailure : kOk;
  });
}

int gannyu_runtime_finalize(void) {
  return AbiStatus([&] {
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!runtime.handles.empty()) {
      SetError("destroy all Rime pipeline handles before finalizing the runtime");
      return kInvalidArgument;
    }
    FinalizeUnusedRuntimeLocked(runtime);
    return kOk;
  });
}

int gannyu_engine_snapshot(GannyuPipelineHandle* handle, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    return WriteSnapshot(handle, false, std::nullopt, out_json);
  });
}

int gannyu_engine_set_candidate_limit(GannyuPipelineHandle* handle, size_t limit) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    handle->candidate_limit = limit;
    return kOk;
  });
}

int gannyu_engine_candidates(GannyuPipelineHandle* handle, size_t offset, size_t limit, char** out_json) {
  return AbiStatus([&] {
    g_last_error.clear();
    if (out_json == nullptr) return kInvalidArgument;
    *out_json = nullptr;
    if (handle == nullptr || limit == 0 || limit > 100 ||
        offset > static_cast<size_t>(std::numeric_limits<int>::max()) - limit) return kInvalidArgument;
    auto& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    if (!RIME_PROVIDED(Api(), candidate_list_from_index) ||
        !RIME_PROVIDED(Api(), candidate_list_next) || !RIME_PROVIDED(Api(), candidate_list_end)) {
      SetError("candidate range iteration is unavailable");
      return kLoadFailure;
    }
    std::ostringstream output;
    output << "{\"rawInput\":\"" << JsonEscape(Api()->get_input(handle->session))
           << "\",\"schemaId\":\"" << JsonEscape(handle->schema_id)
           << "\",\"offset\":" << offset << ",\"candidates\":[";
    bool has_more = false;
    RimeCandidateListIterator iterator{};
    if (Api()->candidate_list_from_index(handle->session, &iterator, static_cast<int>(offset))) {
      auto end = [](RimeCandidateListIterator* value) { Api()->candidate_list_end(value); };
      std::unique_ptr<RimeCandidateListIterator, decltype(end)> guard(&iterator, end);
      size_t count = 0;
      while (count < limit && Api()->candidate_list_next(&iterator)) {
        if (count++ > 0) output << ',';
        const auto& candidate = iterator.candidate;
        output << "{\"text\":\"" << JsonEscape(candidate.text) << "\"";
        if (candidate.comment != nullptr && candidate.comment[0] != '\0') {
          output << ",\"annotation\":\"" << JsonEscape(candidate.comment) << "\"";
        }
        output << ",\"globalIndex\":" << iterator.index
               << ",\"pageIndex\":" << iterator.index << ",\"deletable\":false}";
      }
      has_more = count == limit && Api()->candidate_list_next(&iterator);
    }
    output << "],\"hasMore\":" << (has_more ? "true" : "false") << '}';
    *out_json = CopyOut(output.str());
    return *out_json == nullptr ? kSerializeFailure : kOk;
  });
}

int gannyu_engine_process_key(GannyuPipelineHandle* handle, const char* event_json, char** out_json) {
  return AbiStatus([&] {
    g_last_error.clear();
    if (handle == nullptr || event_json == nullptr) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    const auto type = JsonStringField(event_json, "type");
    if (!type.has_value()) {
      SetError("invalid engine key event");
      return kInvalidArgument;
    }
    bool handled = false;
    std::optional<std::string> commit;
    if (*type == "text") {
      const auto text = JsonStringField(event_json, "text");
      if (!text.has_value()) {
        SetError("text key event is missing text");
        return kInvalidArgument;
      }
      if (IsCompositionText(*text)) {
        handled = true;
        for (const unsigned char key : *text) handled = Api()->process_key(handle->session, key, 0) || handled;
      } else {
        Api()->commit_composition(handle->session);
        commit = TakeCommit(handle->session).value_or("") + *text;
        handled = true;
      }
    } else if (*type == "backspace") {
      handled = Api()->process_key(handle->session, kBackspace, 0);
    } else if (*type == "deleteForward") {
      handled = Api()->process_key(handle->session, kDelete, 0);
    } else if (*type == "moveLeft") {
      handled = Api()->process_key(handle->session, kLeft, 0);
    } else if (*type == "moveRight") {
      handled = Api()->process_key(handle->session, kRight, 0);
    } else if (*type == "space") {
      handled = Api()->process_key(handle->session, ' ', 0);
    } else if (*type == "enter") {
      handled = Api()->process_key(handle->session, kReturn, 0);
    } else {
      SetError("unsupported engine key event: " + *type);
      return kInvalidArgument;
    }
    if (!commit.has_value()) commit = TakeCommit(handle->session);
    return WriteSnapshot(handle, handled, commit, out_json);
  });
}

int gannyu_engine_select_candidate(GannyuPipelineHandle* handle, size_t global_index, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    const bool handled = Api()->select_candidate(handle->session, global_index);
    return WriteSnapshot(handle, handled, TakeCommit(handle->session), out_json);
  });
}

int gannyu_engine_change_page(GannyuPipelineHandle* handle, int direction, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    const bool handled = direction < 0 ? Api()->process_key(handle->session, kPageUp, 0)
                                       : direction > 0 && Api()->process_key(handle->session, kPageDown, 0);
    return WriteSnapshot(handle, handled, TakeCommit(handle->session), out_json);
  });
}

int gannyu_engine_clear_composition(GannyuPipelineHandle* handle, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    Api()->clear_composition(handle->session);
    return WriteSnapshot(handle, true, std::nullopt, out_json);
  });
}

int gannyu_engine_switch_region(GannyuPipelineHandle* handle, const char* region_id, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr || region_id == nullptr || *region_id == '\0') return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    const std::string previous_region = handle->region_id;
    handle->region_id = region_id;
    std::string schema = SchemaFor(*handle);
    bool handled = Api()->select_schema(handle->session, schema.c_str());
    if (!handled && handle->ipa_scheme) {
      handle->ipa_scheme = false;
      schema = SchemaFor(*handle);
      handled = Api()->select_schema(handle->session, schema.c_str());
    }
    if (handled) {
      handle->schema_id = schema;
      ApplyInputOptions(handle);
    } else {
      handle->region_id = previous_region;
    }
    return WriteSnapshot(handle, handled, std::nullopt, out_json);
  });
}

int gannyu_engine_set_ascii_mode(GannyuPipelineHandle* handle, int enabled, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    Api()->set_option(handle->session, "ascii_mode", enabled ? True : False);
    return WriteSnapshot(handle, true, std::nullopt, out_json);
  });
}

int gannyu_engine_transcription_scheme_available(GannyuPipelineHandle* handle, int scheme) {
  return AbiStatus([&] {
    if (handle == nullptr || (scheme != 0 && scheme != 1)) return 0;
    if (scheme == 0) return 1;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return 0;
    RimeSchemaList list{};
    if (!Api()->get_schema_list(&list)) return 0;
    const std::string target = "gannyu_" + handle->region_id + "_ipa";
    bool found = false;
    for (size_t index = 0; index < list.size; ++index) {
      if (list.list != nullptr && list.list[index].schema_id != nullptr &&
          target == list.list[index].schema_id) {
        found = true;
        break;
      }
    }
    Api()->free_schema_list(&list);
    return found ? 1 : 0;
  });
}

int gannyu_engine_set_transcription_scheme(GannyuPipelineHandle* handle, int scheme) {
  return AbiStatus([&] {
    if (handle == nullptr || (scheme != 0 && scheme != 1)) return kInvalidArgument;
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    const bool ipa = scheme == 1;
    if (handle->ipa_scheme == ipa) return kOk;
    handle->ipa_scheme = ipa;
    const std::string schema = SchemaFor(*handle);
    const bool handled = Api()->select_schema(handle->session, schema.c_str());
    if (handled) {
      handle->schema_id = schema;
      ApplyInputOptions(handle);
    } else {
      handle->ipa_scheme = !ipa;
      SetError("second transcription scheme is unavailable for this region");
    }
    return handled ? kOk : kLoadFailure;
  });
}

int gannyu_engine_set_input_options(GannyuPipelineHandle* handle, int annotations,
                                   int relations, int fuzzy, int mandarin,
                                   int fuzzy_rules, int reading_mode) {
  return AbiStatus([&] {
    if (handle == nullptr || reading_mode < 0 || reading_mode > 2) return kInvalidArgument;
    auto& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    const std::array<int, 6> values{{annotations, relations, fuzzy, mandarin, fuzzy_rules, reading_mode}};
    if (handle->input_options == values) return kOk;
    handle->input_options = values;
    ApplyInputOptions(handle);
    return kOk;
  });
}

int gannyu_engine_set_synonyms_enabled(GannyuPipelineHandle* handle, int enabled) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    auto& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    if (handle->synonyms_enabled == (enabled != 0)) return kOk;
    handle->synonyms_enabled = enabled != 0;
    ApplyInputOptions(handle);
    return kOk;
  });
}

int gannyu_engine_set_wenbai_enabled(GannyuPipelineHandle* handle, int enabled) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    auto& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;
    if (handle->wenbai_enabled == (enabled != 0)) return kOk;
    handle->wenbai_enabled = enabled != 0;
    ApplyInputOptions(handle);
    return kOk;
  });
}

int gannyu_engine_reset_user_data(GannyuPipelineHandle* handle, int scope, char** out_json) {
  return AbiStatus([&] {
    g_last_error.clear();
    if (handle == nullptr || out_json == nullptr) return kInvalidArgument;
    *out_json = nullptr;
    if (scope != GANNYU_USER_DATA_ALL) {
      SetError("Rime userdb reset supports complete per-schema reset only");
      return kInvalidArgument;
    }

    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return kInvalidArgument;

    std::vector<GannyuPipelineHandle*> affected;
    for (GannyuPipelineHandle* candidate : runtime.handles) {
      if (candidate->schema_id == handle->schema_id) affected.push_back(candidate);
    }
    std::vector<GannyuPipelineHandle*> stopped;
    stopped.reserve(affected.size());
    for (GannyuPipelineHandle* candidate : affected) {
      if (StopSessionLocked(candidate)) stopped.push_back(candidate);
    }
    if (stopped.size() != affected.size()) {
      bool sessions_restored = true;
      for (GannyuPipelineHandle* candidate : stopped) {
        if (!StartSessionLocked(candidate)) sessions_restored = false;
      }
      if (!sessions_restored) {
        SetError("failed to destroy all Rime sessions; some stopped sessions could not be restored; userdb was not removed");
      } else {
        SetError("failed to destroy all Rime sessions; userdb was not removed");
      }
      return kLoadFailure;
    }

    std::error_code remove_error;
    std::filesystem::path userdb;
    for (const char* suffix : {"", "_both_norel", "_new_rel", "_new_norel", "_old_rel", "_old_norel"}) {
      const auto path = std::filesystem::path(runtime.user_data_dir) / (handle->schema_id + suffix + ".userdb");
      std::error_code error;
      std::filesystem::remove_all(path, error);
      if (error && !remove_error) {
        remove_error = error;
        userdb = path;
      }
    }

    bool sessions_restored = true;
    for (GannyuPipelineHandle* candidate : affected) {
      if (!StartSessionLocked(candidate)) sessions_restored = false;
    }
    if (remove_error) {
      SetError("failed to remove Rime userdb " + userdb.string() + ": " + remove_error.message());
      return kLoadFailure;
    }
    if (!sessions_restored) {
      SetError("Rime userdb was removed but one or more sessions could not be restored");
      return kLoadFailure;
    }
    return WriteSnapshot(handle, true, std::nullopt, out_json);
  });
}

void gannyu_pipeline_destroy(GannyuPipelineHandle* handle) {
  if (handle == nullptr) return;
  try {
    Runtime& runtime = GlobalRuntime();
    std::lock_guard<std::mutex> lock(runtime.mutex);
    if (!HasRegisteredHandleLocked(runtime, handle)) return;
    if (Api() != nullptr) (void)StopSessionLocked(handle);
    runtime.handles.erase(std::remove(runtime.handles.begin(), runtime.handles.end(), handle),
                          runtime.handles.end());
    delete handle;
    FinalizeUnusedRuntimeLocked(runtime);
  } catch (...) {
  }
}

void gannyu_string_destroy(char* value) { std::free(value); }

int gannyu_ffi_status_ok(void) { return kOk; }

}  // extern "C"
