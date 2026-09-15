#include <android/log.h>
#include <jni.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "gannyu_input.h"

#define LOG_TAG "GonnyuNative"
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

static jstring copy_out_string(JNIEnv* env, char* out) {
    if (out == NULL) return NULL;
    jstring result = (*env)->NewStringUTF(env, out);
    gannyu_string_destroy(out);
    return result;
}

static int child_path(char* output, size_t output_size, const char* root, const char* child) {
    if (root == NULL || root[0] == '\0') return -1;
    const int written = snprintf(output, output_size, "%s/%s", root, child);
    return written < 0 || (size_t)written >= output_size ? -1 : 0;
}

JNIEXPORT jlong JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeCreate(
    JNIEnv* env, jobject thiz, jstring resource_root, jstring region, jstring user_data_dir) {
    (void)thiz;
    const char* resource_root_c = resource_root == NULL ? NULL : (*env)->GetStringUTFChars(env, resource_root, NULL);
    const char* region_c = region == NULL ? NULL : (*env)->GetStringUTFChars(env, region, NULL);
    const char* user_data_dir_c = user_data_dir == NULL ? NULL : (*env)->GetStringUTFChars(env, user_data_dir, NULL);
    char shared_data_dir[PATH_MAX];
    char prebuilt_data_dir[PATH_MAX];
    GannyuPipelineHandle* handle = NULL;
    int status = -1;

    if (child_path(shared_data_dir, sizeof(shared_data_dir), resource_root_c, "shared") == 0 &&
        child_path(prebuilt_data_dir, sizeof(prebuilt_data_dir), resource_root_c, "prebuilt") == 0 &&
        user_data_dir_c != NULL && user_data_dir_c[0] != '\0') {
        const GannyuEngineConfig config = {
            sizeof(GannyuEngineConfig), region_c, shared_data_dir, prebuilt_data_dir, user_data_dir_c};
        status = gannyu_engine_create(&config, &handle);
    } else {
        LOGE("nativeCreate requires prepared resource and user-data directories");
    }

    if (resource_root_c != NULL) (*env)->ReleaseStringUTFChars(env, resource_root, resource_root_c);
    if (region_c != NULL) (*env)->ReleaseStringUTFChars(env, region, region_c);
    if (user_data_dir_c != NULL) (*env)->ReleaseStringUTFChars(env, user_data_dir, user_data_dir_c);
    return status == 0 ? (jlong)(intptr_t)handle : 0;
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeLastError(JNIEnv* env, jobject thiz) {
    (void)thiz;
    char* out = NULL;
    return gannyu_last_error(&out) == 0 ? copy_out_string(env, out) : NULL;
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeSnapshot(JNIEnv* env, jobject thiz, jlong handle) {
    (void)thiz;
    char* out = NULL;
    return gannyu_engine_snapshot((GannyuPipelineHandle*)(intptr_t)handle, &out) == 0 ? copy_out_string(env, out) : NULL;
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeProcessKey(
    JNIEnv* env, jobject thiz, jlong handle, jstring event_json) {
    (void)thiz;
    if (event_json == NULL) return NULL;
    const char* event_c = (*env)->GetStringUTFChars(env, event_json, NULL);
    char* out = NULL;
    const int status = event_c == NULL ? -1 :
        gannyu_engine_process_key((GannyuPipelineHandle*)(intptr_t)handle, event_c, &out);
    if (event_c != NULL) (*env)->ReleaseStringUTFChars(env, event_json, event_c);
    return status == 0 ? copy_out_string(env, out) : NULL;
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeSelectCandidate(
    JNIEnv* env, jobject thiz, jlong handle, jint global_index) {
    (void)thiz;
    if (global_index < 0) return NULL;
    char* out = NULL;
    return gannyu_engine_select_candidate((GannyuPipelineHandle*)(intptr_t)handle,
                                           (size_t)global_index, &out) == 0 ? copy_out_string(env, out) : NULL;
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeChangeCandidatePage(
    JNIEnv* env, jobject thiz, jlong handle, jint direction) {
    (void)thiz;
    if (direction == 0) return NULL;
    char* out = NULL;
    return gannyu_engine_change_page((GannyuPipelineHandle*)(intptr_t)handle, direction, &out) == 0
        ? copy_out_string(env, out) : NULL;
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeClearComposition(
    JNIEnv* env, jobject thiz, jlong handle) {
    (void)thiz;
    char* out = NULL;
    return gannyu_engine_clear_composition((GannyuPipelineHandle*)(intptr_t)handle, &out) == 0 ? copy_out_string(env, out) : NULL;
}

JNIEXPORT jboolean JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeResetCurrentUserData(
    JNIEnv* env, jobject thiz, jlong handle) {
    (void)env;
    (void)thiz;
    char* snapshot = NULL;
    const int status = gannyu_engine_reset_user_data((GannyuPipelineHandle*)(intptr_t)handle,
                                                      GANNYU_USER_DATA_ALL, &snapshot);
    gannyu_string_destroy(snapshot);
    return status == 0 ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT void JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeDestroy(JNIEnv* env, jobject thiz, jlong handle) {
    (void)env;
    (void)thiz;
    if (handle != 0) gannyu_pipeline_destroy((GannyuPipelineHandle*)(intptr_t)handle);
}

JNIEXPORT jlong JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeCreate(JNIEnv* env, jobject thiz, jstring resource_root, jstring region, jstring user_data_dir) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeCreate(env, thiz, resource_root, region, user_data_dir);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeLastError(JNIEnv* env, jobject thiz) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeLastError(env, thiz);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeSnapshot(JNIEnv* env, jobject thiz, jlong handle) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeSnapshot(env, thiz, handle);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeProcessKey(JNIEnv* env, jobject thiz, jlong handle, jstring event_json) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeProcessKey(env, thiz, handle, event_json);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeSelectCandidate(JNIEnv* env, jobject thiz, jlong handle, jint global_index) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeSelectCandidate(env, thiz, handle, global_index);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeChangeCandidatePage(JNIEnv* env, jobject thiz, jlong handle, jint direction) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeChangeCandidatePage(env, thiz, handle, direction);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeClearComposition(JNIEnv* env, jobject thiz, jlong handle) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeClearComposition(env, thiz, handle);
}

JNIEXPORT jboolean JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeResetCurrentUserData(JNIEnv* env, jobject thiz, jlong handle) {
    return Java_io_gannyu_input_GannyuInputMethodService_nativeResetCurrentUserData(env, thiz, handle);
}

JNIEXPORT void JNICALL
Java_io_gannyu_input_NativePipelineBridge_nativeDestroy(JNIEnv* env, jobject thiz, jlong handle) {
    Java_io_gannyu_input_GannyuInputMethodService_nativeDestroy(env, thiz, handle);
}
