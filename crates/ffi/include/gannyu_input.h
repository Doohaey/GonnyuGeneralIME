#ifndef GANNYU_INPUT_H
#define GANNYU_INPUT_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct GannyuPipelineHandle GannyuPipelineHandle;

int gannyu_pipeline_create(const char *manifest_path,
                         const char *region_id,
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

void gannyu_string_destroy(char *value);

int gannyu_ffi_status_ok(void);

#ifdef __cplusplus
}
#endif

#endif
