#include "gannyu_input.h"

#include <atomic>
#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>

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
  return failed ? 3 : 0;
}
