#include "gannyu_input.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <initializer_list>
#include <string>

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
    if (!commits_bank_card) return 7;

    const std::string schema_id = std::string("gannyu_") + region;
    const std::filesystem::path userdb = std::filesystem::path(argv[3]) / (schema_id + ".userdb");
    const std::filesystem::path marker = userdb / "gonnyu-reset-probe";
    std::error_code directory_error;
    std::filesystem::create_directories(userdb, directory_error);
    if (directory_error) {
      gannyu_pipeline_destroy(handle);
      return 8;
    }
    std::ofstream(marker) << "reset must remove this file\n";
    snapshot = nullptr;
    if (gannyu_engine_reset_user_data(handle, GANNYU_USER_DATA_ALL, &snapshot) != 0 ||
        snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 9;
    }
    const bool restored_schema = std::strstr(snapshot, schema_id.c_str()) != nullptr;
    std::puts(snapshot);
    gannyu_string_destroy(snapshot);
    if (!restored_schema || std::filesystem::exists(marker)) {
      gannyu_pipeline_destroy(handle);
      return 10;
    }
    gannyu_pipeline_destroy(handle);
  }
  return 0;
}
