#include "gannyu_input.h"

#include <atomic>
#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>

namespace {

GannyuPipelineHandle* CreateHandle(const char* shared, const char* prebuilt, const char* user) {
  GannyuEngineConfig config = GANNYU_ENGINE_CONFIG_INIT;
  config.region_id = "fenni";
  config.shared_data_dir = shared;
  config.prebuilt_data_dir = prebuilt;
  config.user_data_dir = user;
  GannyuPipelineHandle* handle = nullptr;
  return gannyu_engine_create(&config, &handle) == 0 ? handle : nullptr;
}

bool ContainsBankCard(GannyuPipelineHandle* handle) {
  char* snapshot = nullptr;
  if (gannyu_engine_clear_composition(handle, &snapshot) != 0 || snapshot == nullptr) return false;
  gannyu_string_destroy(snapshot);
  snapshot = nullptr;
  const bool ok = gannyu_engine_process_key(handle, "{\"type\":\"text\",\"text\":\"nhk\"}", &snapshot) == 0 &&
                  snapshot != nullptr && std::strstr(snapshot, "银行卡") != nullptr;
  gannyu_string_destroy(snapshot);
  return ok;
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 4) return 2;
  std::atomic<bool> failed = false;
  std::vector<std::thread> workers;
  for (int worker = 0; worker < 4; ++worker) {
    workers.emplace_back([&, worker] {
      for (int iteration = 0; iteration < 3; ++iteration) {
        GannyuEngineConfig config = GANNYU_ENGINE_CONFIG_INIT;
        config.region_id = worker % 2 == 0 ? "fenni" : "lancong";
        config.shared_data_dir = argv[1];
        config.prebuilt_data_dir = argv[2];
        config.user_data_dir = argv[3];
        GannyuPipelineHandle* handle = nullptr;
        if (gannyu_engine_create(&config, &handle) != 0 || handle == nullptr) {
          failed = true;
          return;
        }
        char* snapshot = nullptr;
        if (gannyu_engine_process_key(handle, "{\"type\":\"text\",\"text\":\"nhk\"}", &snapshot) != 0 ||
            snapshot == nullptr || std::strstr(snapshot, "银行卡") == nullptr) {
          failed = true;
        }
        gannyu_string_destroy(snapshot);
        gannyu_pipeline_destroy(handle);
      }
    });
  }
  for (auto& worker : workers) worker.join();
  if (failed) return 3;

  GannyuPipelineHandle* active = CreateHandle(argv[1], argv[2], argv[3]);
  GannyuPipelineHandle* preloaded = CreateHandle(argv[1], argv[2], argv[3]);
  if (active == nullptr || preloaded == nullptr) {
    gannyu_pipeline_destroy(active);
    gannyu_pipeline_destroy(preloaded);
    return 4;
  }
  std::atomic<bool> start_reset = false;
  std::atomic<bool> reset_ok = false;
  std::atomic<bool> query_ok = true;
  std::thread resetter([&] {
    while (!start_reset.load()) std::this_thread::yield();
    char* snapshot = nullptr;
    reset_ok = gannyu_engine_reset_user_data(active, GANNYU_USER_DATA_ALL, &snapshot) == 0 &&
               snapshot != nullptr;
    gannyu_string_destroy(snapshot);
  });
  std::thread reader([&] {
    start_reset = true;
    for (int iteration = 0; iteration < 4; ++iteration) {
      char* snapshot = nullptr;
      if (gannyu_engine_snapshot(preloaded, &snapshot) != 0 || snapshot == nullptr) query_ok = false;
      gannyu_string_destroy(snapshot);
    }
  });
  resetter.join();
  reader.join();
  if (!reset_ok || !query_ok) {
    gannyu_pipeline_destroy(active);
    gannyu_pipeline_destroy(preloaded);
    return 5;
  }
  const bool active_restored = ContainsBankCard(active);
  const bool preload_restored = ContainsBankCard(preloaded);
  gannyu_pipeline_destroy(active);
  gannyu_pipeline_destroy(preloaded);
  return active_restored && preload_restored ? 0 : 6;
}
