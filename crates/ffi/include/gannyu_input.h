#ifndef GANNYU_INPUT_H
#define GANNYU_INPUT_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct GannyuPipelineHandle GannyuPipelineHandle;

typedef struct GannyuEngineConfig {
    size_t struct_size;
    const char *region_id;
    const char *shared_data_dir;
    const char *prebuilt_data_dir;
    const char *user_data_dir;
} GannyuEngineConfig;

#define GANNYU_ENGINE_CONFIG_INIT \
    { sizeof(GannyuEngineConfig), NULL, NULL, NULL, NULL }

/*
 * Ownership and threading contract shared by the Rust and Rime engines:
 *
 * - A successful create call transfers one opaque handle to the caller.
 * - The caller must release it exactly once with gannyu_pipeline_destroy().
 * - Engine calls may originate on different threads and are serialized by the
 *   implementation. Destroy is the exception: the caller must first stop and
 *   join all operations that could still use the handle.
 * - Every non-NULL char** result is owned by the caller and must be released
 *   exactly once with gannyu_string_destroy(). Never use free() directly.
 * - Returned snapshots are immutable copies; no pointer aliases engine state.
 * - Errors are thread-local and consumed by gannyu_last_error().
 * - No C++ or Rust exception/panic may cross this C ABI boundary.
 */

int gannyu_engine_create(const GannyuEngineConfig *config,
                         GannyuPipelineHandle **out_handle);

int gannyu_pipeline_create(const char *manifest_path,
                         const char *region_id,
                         GannyuPipelineHandle **out_handle);

int gannyu_pipeline_create_with_user_data_dir(const char *manifest_path,
                                               const char *region_id,
                                               const char *user_data_dir,
                                               GannyuPipelineHandle **out_handle);

int gannyu_last_error(char **out_error);

int gannyu_pipeline_compose(GannyuPipelineHandle *handle,
                          const char *input,
                          char **out_json);

int gannyu_pipeline_retrieve(GannyuPipelineHandle *handle,
                           const char *input,
                           char **out_json);

int gannyu_pipeline_segment_sentence(GannyuPipelineHandle *handle,
                                   const char *input,
                                   char **out_json);

int gannyu_pipeline_segment_boundaries(GannyuPipelineHandle *handle,
                                    const char *input,
                                    char **out_json);

int gannyu_pipeline_format_preedit(GannyuPipelineHandle *handle,
                                 const char *input,
                                 size_t consumed_bytes,
                                 char **out_text);

int gannyu_pipeline_user_dict_add(GannyuPipelineHandle *handle,
                                const char *headword,
                                const char *pinyin,
                                const char *mandarin_pinyin,
                                char **out_json);

int gannyu_pipeline_user_dict_boost(GannyuPipelineHandle *handle,
                                  const char *headword,
                                  char **out_json);

enum {
    GANNYU_USER_DATA_WORDS = 1,
    GANNYU_USER_DATA_FREQUENCIES = 2,
    GANNYU_USER_DATA_ALL = 3
};

int gannyu_pipeline_user_data_clear(GannyuPipelineHandle *handle, int scope);

void gannyu_pipeline_destroy(GannyuPipelineHandle *handle);

int gannyu_pipeline_entry_count(GannyuPipelineHandle *handle);

int gannyu_region_list(const char *manifest_path, char **out_json);

int gannyu_engine_snapshot(GannyuPipelineHandle *handle, char **out_json);
int gannyu_engine_set_candidate_limit(GannyuPipelineHandle *handle, size_t limit);

int gannyu_engine_process_key(GannyuPipelineHandle *handle,
                            const char *event_json,
                            char **out_json);

int gannyu_engine_select_candidate(GannyuPipelineHandle *handle,
                                 size_t global_index,
                                 char **out_json);

int gannyu_engine_change_page(GannyuPipelineHandle *handle,
                            int direction,
                            char **out_json);

int gannyu_engine_clear_composition(GannyuPipelineHandle *handle, char **out_json);

int gannyu_engine_switch_region(GannyuPipelineHandle *handle,
                              const char *region_id,
                              char **out_json);

int gannyu_engine_set_ascii_mode(GannyuPipelineHandle *handle,
                               int enabled,
                               char **out_json);

int gannyu_engine_reset_user_data(GannyuPipelineHandle *handle,
                                int scope,
                                char **out_json);

void gannyu_string_destroy(char *value);

int gannyu_ffi_status_ok(void);

#ifdef __cplusplus
}
#endif

#endif
