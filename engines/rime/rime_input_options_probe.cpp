#include "gannyu_input.h"

#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>

int main(int argc, char** argv) {
  if (argc != 4) return 2;
  GannyuEngineConfig config = GANNYU_ENGINE_CONFIG_INIT;
  config.region_id = "lancong";
  config.shared_data_dir = argv[1];
  config.prebuilt_data_dir = argv[2];
  config.user_data_dir = argv[3];
  GannyuPipelineHandle* handle = nullptr;
  GannyuPipelineHandle* second = nullptr;
  auto check = [](bool result, const char* message) {
    if (!result) throw std::runtime_error(message);
  };
  auto snapshot = [&](const char* event = nullptr) {
    char* json = nullptr;
    const int status = event ? gannyu_engine_process_key(handle, event, &json)
                             : gannyu_engine_snapshot(handle, &json);
    check(status == 0 && json != nullptr, "snapshot failed");
    std::string result(json);
    gannyu_string_destroy(json);
    return result;
  };
  auto input = [&](const char* text) {
    char* cleared = nullptr;
    check(gannyu_engine_clear_composition(handle, &cleared) == 0, "clear failed");
    gannyu_string_destroy(cleared);
    const std::string event = std::string("{\"type\":\"text\",\"text\":\"") + text + "\"}";
    return snapshot(event.c_str());
  };
  try {
    check(gannyu_engine_create(&config, &handle) == 0, "create failed");
    check(gannyu_engine_create(&config, &second) == 0, "second session failed");
    char* second_snapshot = nullptr;
    check(gannyu_engine_process_key(second, "{\"type\":\"text\",\"text\":\"chema\"}", &second_snapshot) == 0, "second session input failed");
    gannyu_string_destroy(second_snapshot);
    check(gannyu_engine_set_candidate_limit(handle, 100) == 0, "candidate limit failed");
    auto result = snapshot("{\"type\":\"text\",\"text\":\"chema\"}");
    check(result.find("\"text\":\"车马\"") != std::string::npos, "source Mandarin lookup missing");
    check(gannyu_engine_set_input_options(handle, 1, 1, 0, 1, 1, 0) == 0, "fuzzy toggle failed");
    result = snapshot();
    check(result.find("\"rawInput\":\"chema\"") != std::string::npos, "hot toggle lost input");
    check(result.find("\"text\":\"车马\"") == std::string::npos, "disabled Mandarin lookup leaked");
    second_snapshot = nullptr;
    check(gannyu_engine_snapshot(second, &second_snapshot) == 0 && second_snapshot != nullptr, "second snapshot failed");
    const std::string independent(second_snapshot);
    gannyu_string_destroy(second_snapshot);
    check(independent.find("\"text\":\"车马\"") != std::string::npos, "options leaked into another live session");
    gannyu_pipeline_destroy(second);
    second = nullptr;
    check(gannyu_engine_set_input_options(handle, 1, 1, 1, 1, 1, 0) == 0, "restore failed");
    check(snapshot().find("\"text\":\"车马\"") != std::string::npos, "restored lookup missing");
    check(gannyu_engine_set_input_options(handle, 0, 1, 1, 1, 1, 0) == 0, "annotation toggle failed");
    result = snapshot();
    check(result.find("\"rawInput\":\"chema\"") != std::string::npos, "annotation toggle lost input");
    check(result.find("\"annotation\"") == std::string::npos, "hidden annotations leaked");
    check(result.find("\"text\":\"车马\"") != std::string::npos, "Gan word lost after annotation toggle");
    check(gannyu_engine_set_input_options(handle, 1, 1, 1, 1, 1, 0) == 0, "final restore failed");
    check(snapshot().find("\"annotation\"") != std::string::npos, "restored annotations missing");
    for (int relations : {0, 1}) {
      for (int synonyms : {0, 1}) {
        check(gannyu_engine_set_input_options(handle, 1, relations, 1, 1, 1, 0) == 0, "relation setting failed");
        check(gannyu_engine_set_synonyms_enabled(handle, synonyms) == 0, "synonym setting failed");
        result = input("jido");
        check(result.find("\"text\":\"几多\"") != std::string::npos, "source synonym headword missing");
        check((result.find("\"text\":\"多少\"") != std::string::npos) == (synonyms != 0), "synonym candidate dependency failed");
        check((result.find("[联]多少") != std::string::npos) == (synonyms != 0), "synonym annotation dependency failed");
        check(gannyu_engine_set_synonyms_enabled(handle, !synonyms) == 0, "hot synonym toggle failed");
        result = snapshot();
        check(result.find("\"rawInput\":\"jido\"") != std::string::npos, "synonym toggle lost input");
        check((result.find("\"text\":\"多少\"") != std::string::npos) == (synonyms == 0), "hot synonym toggle failed to refresh");
        check(input("doseu").find("\"text\":\"多少\"") != std::string::npos, "independent synonym lookup lost");
        result = input("za");
        check(result.find("\"text\":\"这\"") != std::string::npos, "Mandarin character was filtered");
        if (!relations) {
          check(result.find("[不习用]") == std::string::npos, "Mandarin character usage label leaked");
          check(input("yangniu").find("放牛") == std::string::npos, "Mandarin word leaked with synonyms enabled");
        }
      }
    }
    check(gannyu_engine_set_synonyms_enabled(handle, 1) == 0, "restore synonyms failed");
    check(gannyu_engine_set_input_options(handle, 0, 1, 1, 1, 1, 0) == 0, "synonym parent toggle failed");
    result = input("jido");
    check(result.find("\"text\":\"多少\"") != std::string::npos, "hidden annotations disabled independent synonyms");
    check(result.find("\"annotation\"") == std::string::npos, "synonym annotations leaked while hidden");
    check(input("yangniu").find("\"text\":\"放牛\"") != std::string::npos, "hidden annotations disabled independent Gan Mandarin relation");
    check(gannyu_engine_set_input_options(handle, 1, 1, 1, 1, 1, 0) == 0, "restore parent failed");
    check(input("jido").find("\"text\":\"多少\"") != std::string::npos, "annotation toggle lost saved synonym setting");
    check(gannyu_engine_set_input_options(handle, 1, 1, 1, 0, 0, 0) == 0, "isolate WenBai options failed");
    check(gannyu_engine_set_wenbai_enabled(handle, 1) == 0, "enable WenBai failed");
    check(input("jiangji").find("\"text\":\"经济\"") != std::string::npos, "literary word missing through colloquial reading");
    check(input("lenkieu").find("\"text\":\"人口\"") != std::string::npos, "colloquial word missing through literary reading");
    result = input("jiangji");
    check(gannyu_engine_set_wenbai_enabled(handle, 0) == 0, "disable WenBai failed");
    result = snapshot();
    check(result.find("\"rawInput\":\"jiangji\"") != std::string::npos, "WenBai toggle lost input");
    check(result.find("\"text\":\"经济\"") == std::string::npos, "disabled WenBai expansion leaked");
    check(input("ninkieu").find("\"text\":\"人口\"") != std::string::npos, "own colloquial word reading was lost");
    check(input("jinji").find("\"text\":\"经济\"") != std::string::npos, "own word reading lost after WenBai toggle");
    result = input("jin");
    check(result.find("[文]") != std::string::npos && result.find("[白]") != std::string::npos, "WenBai labels were hidden");
    check(gannyu_engine_set_input_options(handle, 1, 1, 1, 1, 0, 0) == 0, "enable independent Mandarin lookup failed");
    check(input("jingji").find("\"text\":\"经济\"") != std::string::npos, "WenBai setting blocked independent Mandarin lookup");
    check(gannyu_engine_set_wenbai_enabled(handle, 1) == 0, "restore WenBai failed");
    check(gannyu_engine_set_input_options(handle, 1, 1, 0, 0, 0, 0) == 0, "disable fuzzy parent failed");
    check(input("jiangji").find("\"text\":\"经济\"") == std::string::npos, "fuzzy parent failed to disable WenBai");
    check(gannyu_engine_set_input_options(handle, 1, 1, 1, 1, 1, 0) == 0, "restore fuzzy parent failed");
    check(snapshot().find("\"text\":\"经济\"") != std::string::npos, "fuzzy parent lost saved WenBai setting");
    result = input("yangniu");
    check(result.find("\"text\":\"放牛\"") != std::string::npos, "source relation missing");
    check(gannyu_engine_set_input_options(handle, 1, 0, 1, 1, 1, 0) == 0, "relation toggle failed");
    result = snapshot();
    check(result.find("\"text\":\"䁐牛\"") != std::string::npos, "source Gan word missing");
    check(result.find("放牛") == std::string::npos, "disabled Mandarin word or annotation leaked");
    check(input("fangniu").find("\"text\":\"放牛\"") == std::string::npos, "direct Mandarin word lookup leaked");
    check(gannyu_engine_set_input_options(handle, 1, 1, 0, 1, 1, 1) == 0, "new mode failed");
    result = input("yi");
    check(result.find("\"text\":\"义\"") != std::string::npos, "source new reading missing");
    check(result.find("[老]") == std::string::npos, "old annotation leaked into new mode");
    check(input("ni").find("\"text\":\"义\"") == std::string::npos, "old lookup leaked into new mode");
    check(gannyu_engine_set_input_options(handle, 1, 1, 0, 1, 1, 2) == 0, "old mode failed");
    check(snapshot().find("\"text\":\"义\"") != std::string::npos, "source old reading missing after hot toggle");
    check(input("yi").find("\"text\":\"义\"") == std::string::npos, "new lookup leaked into old mode");
    for (const char* base : {"gannyu_lancong", "konny_lancong_II", "gannyu_lancong_II", "gannyu_lancong_ipa"}) {
      for (const char* suffix : {"", "_both_norel", "_new_rel", "_new_norel", "_old_rel", "_old_norel"}) {
        const auto path = std::filesystem::path(argv[3]) / (std::string(base) + suffix + ".userdb");
        std::filesystem::create_directories(path);
        std::ofstream(path / "input-options-cleanup-marker").put('1');
      }
    }
    char* reset = nullptr;
    check(gannyu_engine_reset_user_data(handle, GANNYU_USER_DATA_ALL, &reset) == 0, "reset failed");
    gannyu_string_destroy(reset);
    for (const char* base : {"gannyu_lancong", "konny_lancong_II", "gannyu_lancong_II", "gannyu_lancong_ipa"}) {
      for (const char* suffix : {"", "_both_norel", "_new_rel", "_new_norel", "_old_rel", "_old_norel"}) {
        const auto marker = std::filesystem::path(argv[3]) / (std::string(base) + suffix + ".userdb") / "input-options-cleanup-marker";
        check(!std::filesystem::exists(marker), "variant user data survived reset");
      }
    }
    check(input("ni").find("\"text\":\"义\"") != std::string::npos, "reset lost reading mode");
    gannyu_pipeline_destroy(handle);
    std::cout << "hot input options preserve input and update matching and annotations\n";
    return 0;
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    if (handle) gannyu_pipeline_destroy(handle);
    if (second) gannyu_pipeline_destroy(second);
    return 1;
  }
}
