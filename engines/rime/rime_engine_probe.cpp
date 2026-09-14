#include "gannyu_input.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <initializer_list>

int main(int argc, char** argv) {
  if (argc != 4) {
    std::fprintf(stderr, "usage: %s SHARED_DATA_DIR PREBUILT_DATA_DIR USER_DATA_DIR\n", argv[0]);
    return 2;
  }
  for (const char* region : {"fenni", "lancong"}) {
    GannyuPipelineHandle* handle = nullptr;
    GannyuEngineConfig config = GANNYU_ENGINE_CONFIG_INIT;
    config.region_id = region;
    config.shared_data_dir = argv[1];
    config.prebuilt_data_dir = argv[2];
    config.user_data_dir = argv[3];
    if (gannyu_engine_create(&config, &handle) != 0 || handle == nullptr) {
      return 3;
    }
    char* snapshot = nullptr;
    const int status = gannyu_engine_process_key(handle, "{\"type\":\"text\",\"text\":\"nhk\"}", &snapshot);
    if (status != 0 || snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 4;
    }
    const bool has_bank_card = std::strstr(snapshot, "银行卡") != nullptr;
    std::puts(snapshot);
    gannyu_string_destroy(snapshot);
    if (!has_bank_card) return 5;
    snapshot = nullptr;
    if (gannyu_engine_select_candidate(handle, 0, &snapshot) != 0 || snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 6;
    }
    const bool commits_bank_card = std::strstr(snapshot, "\"commitText\":\"银行卡\"") != nullptr;
    std::puts(snapshot);
    gannyu_string_destroy(snapshot);
    gannyu_pipeline_destroy(handle);
    if (!commits_bank_card) return 7;
  }
  return 0;
}
