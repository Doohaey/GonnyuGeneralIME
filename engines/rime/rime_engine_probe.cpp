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
    if (gannyu_engine_process_key(handle, "{\"type\":\"text\",\"text\":\"gau\"}", &snapshot) != 0 ||
        snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 4;
    }
    gannyu_string_destroy(snapshot);
    snapshot = nullptr;
    if (gannyu_engine_process_key(handle, "{\"type\":\"moveLeft\"}", &snapshot) != 0 ||
        snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 5;
    }
    gannyu_string_destroy(snapshot);
    snapshot = nullptr;
    if (gannyu_engine_process_key(handle, "{\"type\":\"text\",\"text\":\"i\"}", &snapshot) != 0 ||
        snapshot == nullptr || std::strstr(snapshot, "\"rawInput\":\"gaiu\"") == nullptr ||
        std::strstr(snapshot, "\"caret\":3") == nullptr) {
      gannyu_string_destroy(snapshot);
      gannyu_pipeline_destroy(handle);
      return 6;
    }
    gannyu_string_destroy(snapshot);
    snapshot = nullptr;
    if (gannyu_engine_clear_composition(handle, &snapshot) != 0 || snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 7;
    }
    gannyu_string_destroy(snapshot);
    snapshot = nullptr;
    const int status = gannyu_engine_process_key(handle, "{\"type\":\"text\",\"text\":\"nhk\"}", &snapshot);
    if (status != 0 || snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 8;
    }
    const bool has_bank_card = std::strstr(snapshot, "银行卡") != nullptr;
    std::puts(snapshot);
    gannyu_string_destroy(snapshot);
    if (!has_bank_card) return 9;
    snapshot = nullptr;
    if (gannyu_engine_select_candidate(handle, 0, &snapshot) != 0 || snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 10;
    }
    const bool commits_bank_card = std::strstr(snapshot, "\"commitText\":\"银行卡\"") != nullptr;
    std::puts(snapshot);
    gannyu_string_destroy(snapshot);
    if (!commits_bank_card) return 11;

    if (std::strcmp(region, "lancong") == 0) {
      if (gannyu_engine_set_candidate_limit(handle, 100) != 0) {
        gannyu_pipeline_destroy(handle);
        return 18;
      }
      snapshot = nullptr;
      if (gannyu_engine_process_key(handle, "{\"type\":\"text\",\"text\":\"birt\"}", &snapshot) != 0 ||
          snapshot == nullptr) {
        gannyu_pipeline_destroy(handle);
        return 16;
      }
      const bool has_not = std::strstr(snapshot, "\"text\":\"不\"") != nullptr;
      const bool has_north = std::strstr(snapshot, "\"text\":\"北\"") != nullptr;
      std::puts(snapshot);
      gannyu_string_destroy(snapshot);
      if (!has_not || has_north) {
        gannyu_pipeline_destroy(handle);
        return 17;
      }
    }

    const std::string schema_id = std::string("gannyu_") + region;
    const std::filesystem::path root = std::filesystem::path(argv[3]);
    const std::filesystem::path gon_marker = root / (schema_id + ".userdb") / "gonnyu-reset-probe";
    const std::filesystem::path ii_marker = root / (schema_id + "_II.userdb") / "gonnyu-reset-probe";
    const std::filesystem::path legacy_marker = root / (schema_id + "_ipa.userdb") / "gonnyu-reset-probe";
    std::error_code directory_error;
    for (const auto& marker : {gon_marker, ii_marker, legacy_marker}) {
      std::filesystem::create_directories(marker.parent_path(), directory_error);
      if (directory_error) {
        gannyu_pipeline_destroy(handle);
        return 12;
      }
      std::ofstream(marker) << "reset must remove this file\n";
    }
    snapshot = nullptr;
    if (gannyu_engine_reset_user_data(handle, GANNYU_USER_DATA_ALL, &snapshot) != 0 ||
        snapshot == nullptr) {
      gannyu_pipeline_destroy(handle);
      return 13;
    }
    const bool restored_schema = std::strstr(snapshot, schema_id.c_str()) != nullptr;
    std::puts(snapshot);
    gannyu_string_destroy(snapshot);
    if (!restored_schema || std::filesystem::exists(gon_marker) ||
        std::filesystem::exists(ii_marker) || std::filesystem::exists(legacy_marker)) {
      gannyu_pipeline_destroy(handle);
      return 14;
    }
    gannyu_pipeline_destroy(handle);
    if (gannyu_runtime_finalize() != 0) return 15;
  }
  return 0;
}
