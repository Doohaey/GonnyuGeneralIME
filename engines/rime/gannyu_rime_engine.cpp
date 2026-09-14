#include "gannyu_input.h"

#include <rime_api.h>

#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <filesystem>
#include <memory>
#include <mutex>
#include <optional>
#include <sstream>
#include <string>
#include <vector>

struct GannyuPipelineHandle {
  RimeSessionId session = 0;
  std::string schema_id;
};

namespace {

constexpr int kOk = 0;
constexpr int kInvalidArgument = 1;
constexpr int kLoadFailure = 2;
constexpr int kSerializeFailure = 3;
constexpr int kBackspace = 0xff08;
constexpr int kReturn = 0xff0d;
constexpr int kPageUp = 0xff55;
constexpr int kPageDown = 0xff56;

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

bool EnsureRuntime(const char* shared_data_dir, const char* prebuilt_data_dir, const char* user_data_dir) {
  if (shared_data_dir == nullptr || *shared_data_dir == '\0' ||
      prebuilt_data_dir == nullptr || *prebuilt_data_dir == '\0' ||
      user_data_dir == nullptr || *user_data_dir == '\0') {
    SetError("Rime requires explicit shared, prebuilt, and user data directories");
    return false;
  }
  Runtime& runtime = GlobalRuntime();
  std::lock_guard<std::mutex> lock(runtime.mutex);
  if (runtime.initialized) {
    if (runtime.shared_data_dir != shared_data_dir ||
        runtime.prebuilt_data_dir != prebuilt_data_dir ||
        runtime.user_data_dir != user_data_dir) {
      SetError("librime is process-global; all sessions must use the same data directories");
      return false;
    }
    return true;
  }
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
  runtime.api->initialize(nullptr);
  runtime.initialized = true;
  return true;
}

RimeApi* Api() { return GlobalRuntime().api; }

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
  return true;
}

void StopSessionLocked(GannyuPipelineHandle* handle) {
  if (handle->session == 0) return;
  Api()->destroy_session(handle->session);
  handle->session = 0;
}

std::optional<std::string> TakeCommit(RimeSessionId session) {
  RimeCommit commit{};
  RIME_STRUCT_INIT(RimeCommit, commit);
  if (!Api()->get_commit(session, &commit)) return std::nullopt;
  std::string text = commit.text ? commit.text : "";
  Api()->free_commit(&commit);
  return text;
}

std::string Snapshot(GannyuPipelineHandle* handle, bool handled, const std::optional<std::string>& commit) {
  RimeContext context{};
  RIME_STRUCT_INIT(RimeContext, context);
  RimeStatus status{};
  RIME_STRUCT_INIT(RimeStatus, status);
  const bool has_context = Api()->get_context(handle->session, &context);
  const bool has_status = Api()->get_status(handle->session, &status);
  std::ostringstream output;
  const char* raw_input = Api()->get_input(handle->session);
  output << "{\"handled\":" << (handled ? "true" : "false") << ",\"rawInput\":\"";
  const char* preedit = has_context && context.composition.preedit ? context.composition.preedit : "";
  output << JsonEscape(raw_input) << "\",\"preedit\":\"" << JsonEscape(preedit) << "\",\"caret\":";
  output << Utf8CountBefore(raw_input, Api()->get_caret_pos(handle->session));
  if (commit.has_value() && !commit->empty()) output << ",\"commitText\":\"" << JsonEscape(*commit) << "\"";
  output << ",\"candidates\":[";
  if (has_context) {
    for (int index = 0; index < context.menu.num_candidates; ++index) {
      if (index != 0) output << ',';
      const RimeCandidate& candidate = context.menu.candidates[index];
      output << "{\"text\":\"" << JsonEscape(candidate.text) << "\"";
      if (candidate.comment != nullptr && candidate.comment[0] != '\0') {
        output << ",\"annotation\":\"" << JsonEscape(candidate.comment) << "\"";
      }
      output << ",\"globalIndex\":" << context.menu.page_no * context.menu.page_size + index;
      output << ",\"pageIndex\":" << index << ",\"deletable\":false}";
    }
  }
  output << "]";
  if (has_context && context.menu.num_candidates > 0) {
    output << ",\"highlightedIndex\":" << context.menu.highlighted_candidate_index;
  }
  output << ",\"pageNumber\":" << (has_context ? context.menu.page_no : 0);
  output << ",\"hasPreviousPage\":" << (has_context && context.menu.page_no > 0 ? "true" : "false");
  output << ",\"hasNextPage\":"
         << (has_context && context.menu.num_candidates > 0 && !context.menu.is_last_page ? "true" : "false");
  output << ",\"schemaId\":\"" << JsonEscape(handle->schema_id) << "\"";
  output << ",\"asciiMode\":" << (has_status && status.is_ascii_mode ? "true" : "false") << '}';
  if (has_context) Api()->free_context(&context);
  if (has_status) Api()->free_status(&status);
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
  if (!EnsureRuntime(shared, prebuilt_data_dir, user_data_dir)) return kLoadFailure;
  Runtime& runtime = GlobalRuntime();
  std::lock_guard<std::mutex> runtime_lock(runtime.mutex);
  auto handle = std::make_unique<GannyuPipelineHandle>();
  const std::string region = region_id && *region_id ? region_id : "lancong";
  handle->schema_id = "gannyu_" + region;
  if (!StartSessionLocked(handle.get())) return kLoadFailure;
  try {
    runtime.handles.push_back(handle.get());
  } catch (...) {
    StopSessionLocked(handle.get());
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

int gannyu_engine_snapshot(GannyuPipelineHandle* handle, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    std::lock_guard<std::mutex> lock(GlobalRuntime().mutex);
    return WriteSnapshot(handle, false, std::nullopt, out_json);
  });
}

int gannyu_engine_process_key(GannyuPipelineHandle* handle, const char* event_json, char** out_json) {
  return AbiStatus([&] {
    g_last_error.clear();
    if (handle == nullptr || event_json == nullptr) return kInvalidArgument;
    std::lock_guard<std::mutex> lock(GlobalRuntime().mutex);
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
    std::lock_guard<std::mutex> lock(GlobalRuntime().mutex);
    const bool handled = Api()->select_candidate(handle->session, global_index);
    return WriteSnapshot(handle, handled, TakeCommit(handle->session), out_json);
  });
}

int gannyu_engine_change_page(GannyuPipelineHandle* handle, int direction, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    std::lock_guard<std::mutex> lock(GlobalRuntime().mutex);
    const bool handled = direction < 0 ? Api()->process_key(handle->session, kPageUp, 0)
                                       : direction > 0 && Api()->process_key(handle->session, kPageDown, 0);
    return WriteSnapshot(handle, handled, TakeCommit(handle->session), out_json);
  });
}

int gannyu_engine_clear_composition(GannyuPipelineHandle* handle, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    std::lock_guard<std::mutex> lock(GlobalRuntime().mutex);
    Api()->clear_composition(handle->session);
    return WriteSnapshot(handle, true, std::nullopt, out_json);
  });
}

int gannyu_engine_switch_region(GannyuPipelineHandle* handle, const char* region_id, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr || region_id == nullptr || *region_id == '\0') return kInvalidArgument;
    std::lock_guard<std::mutex> lock(GlobalRuntime().mutex);
    const std::string schema = "gannyu_" + std::string(region_id);
    const bool handled = Api()->select_schema(handle->session, schema.c_str());
    if (handled) handle->schema_id = schema;
    return WriteSnapshot(handle, handled, std::nullopt, out_json);
  });
}

int gannyu_engine_set_ascii_mode(GannyuPipelineHandle* handle, int enabled, char** out_json) {
  return AbiStatus([&] {
    if (handle == nullptr) return kInvalidArgument;
    std::lock_guard<std::mutex> lock(GlobalRuntime().mutex);
    Api()->set_option(handle->session, "ascii_mode", enabled ? True : False);
    return WriteSnapshot(handle, true, std::nullopt, out_json);
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
    const auto registered = std::find(runtime.handles.begin(), runtime.handles.end(), handle);
    if (registered == runtime.handles.end()) {
      SetError("engine handle is not registered with the Rime runtime");
      return kInvalidArgument;
    }

    std::vector<GannyuPipelineHandle*> affected;
    for (GannyuPipelineHandle* candidate : runtime.handles) {
      if (candidate->schema_id == handle->schema_id) affected.push_back(candidate);
    }
    for (GannyuPipelineHandle* candidate : affected) StopSessionLocked(candidate);

    std::error_code remove_error;
    const std::filesystem::path userdb =
        std::filesystem::path(runtime.user_data_dir) / (handle->schema_id + ".userdb");
    std::filesystem::remove_all(userdb, remove_error);

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
    if (Api() != nullptr) StopSessionLocked(handle);
    runtime.handles.erase(std::remove(runtime.handles.begin(), runtime.handles.end(), handle),
                          runtime.handles.end());
    delete handle;
  } catch (...) {
  }
}

void gannyu_string_destroy(char* value) { std::free(value); }

int gannyu_ffi_status_ok(void) { return kOk; }

}  // extern "C"
