#include <jni.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <android/log.h>
#include "gannyu_input.h"

#define LOG_TAG "GannyuNative"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

static jstring copy_out_string(JNIEnv* env, char* out) {
    if (out == NULL) {
        return NULL;
    }
    jstring result = (*env)->NewStringUTF(env, out);
    gannyu_string_destroy(out);
    return result;
}

JNIEXPORT jlong JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeCreate(
    JNIEnv* env,
    jobject thiz,
    jstring manifest,
    jstring region,
    jstring data_dir
) {
    (void)thiz;
    const char* manifest_c = manifest ? (*env)->GetStringUTFChars(env, manifest, NULL) : NULL;
    const char* region_c = region ? (*env)->GetStringUTFChars(env, region, NULL) : NULL;
    const char* data_dir_c = data_dir ? (*env)->GetStringUTFChars(env, data_dir, NULL) : NULL;
    LOGI("nativeCreate: manifest=%s region=%s data_dir=%s",
         manifest_c ? manifest_c : "(null)",
         region_c ? region_c : "(null)",
         data_dir_c ? data_dir_c : "(null)");
    if (data_dir_c && data_dir_c[0] != '\0') {
        setenv("GANNYU_DATA_HOME", data_dir_c, 1);
        setenv("TMPDIR", data_dir_c, 1);
    }
    GannyuPipelineHandle* handle = NULL;
    LOGI("nativeCreate: calling gannyu_pipeline_create...");
    int status = gannyu_pipeline_create(manifest_c, region_c, &handle);
    LOGI("nativeCreate: gannyu_pipeline_create returned status=%d handle=%p", status, handle);
    if (manifest_c) (*env)->ReleaseStringUTFChars(env, manifest, manifest_c);
    if (region_c) (*env)->ReleaseStringUTFChars(env, region, region_c);
    if (data_dir_c) (*env)->ReleaseStringUTFChars(env, data_dir, data_dir_c);
    return status == 0 ? (jlong)(intptr_t)handle : 0;
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeLastError(
    JNIEnv* env,
    jobject thiz
) {
    (void)thiz;
    char* out = NULL;
    int status = gannyu_last_error(&out);
    if (status != 0 || out == NULL) return NULL;
    return copy_out_string(env, out);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeRegionList(
    JNIEnv* env,
    jobject thiz,
    jstring manifest
) {
    (void)thiz;
    const char* manifest_c = manifest ? (*env)->GetStringUTFChars(env, manifest, NULL) : NULL;
    char* out = NULL;
    int status = gannyu_region_list(manifest_c, &out);
    if (manifest_c) (*env)->ReleaseStringUTFChars(env, manifest, manifest_c);
    if (status != 0 || out == NULL) return NULL;
    return copy_out_string(env, out);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeRetrieve(
    JNIEnv* env,
    jobject thiz,
    jlong handle,
    jstring input
) {
    (void)thiz;
    const char* input_c = (*env)->GetStringUTFChars(env, input, NULL);
    char* out = NULL;
    int status = gannyu_pipeline_retrieve((void*)(intptr_t)handle, input_c, &out);
    (*env)->ReleaseStringUTFChars(env, input, input_c);
    if (status != 0 || out == NULL) return NULL;
    return copy_out_string(env, out);
}

JNIEXPORT jstring JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeFormatPreedit(
    JNIEnv* env,
    jobject thiz,
    jlong handle,
    jstring input,
    jint consumed_bytes
) {
    (void)thiz;
    const char* input_c = (*env)->GetStringUTFChars(env, input, NULL);
    char* out = NULL;
    int status = gannyu_pipeline_format_preedit((void*)(intptr_t)handle, input_c,
                                                  consumed_bytes < 0 ? 0 : (size_t)consumed_bytes,
                                                  &out);
    (*env)->ReleaseStringUTFChars(env, input, input_c);
    if (status != 0 || out == NULL) return NULL;
    return copy_out_string(env, out);
}

JNIEXPORT jint JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeEntryCount(
    JNIEnv* env,
    jobject thiz,
    jlong handle
) {
    (void)env;
    (void)thiz;
    return gannyu_pipeline_entry_count((void*)(intptr_t)handle);
}

JNIEXPORT jboolean JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeUserDictAdd(
    JNIEnv* env,
    jobject thiz,
    jlong handle,
    jstring headword,
    jstring pinyin,
    jstring mandarin_pinyin
) {
    (void)thiz;
    const char* headword_c = (*env)->GetStringUTFChars(env, headword, NULL);
    const char* pinyin_c = (*env)->GetStringUTFChars(env, pinyin, NULL);
    const char* mandarin_c = (*env)->GetStringUTFChars(env, mandarin_pinyin, NULL);
    int status = gannyu_pipeline_user_dict_add(
        (void*)(intptr_t)handle,
        headword_c,
        pinyin_c,
        mandarin_c,
        NULL
    );
    (*env)->ReleaseStringUTFChars(env, headword, headword_c);
    (*env)->ReleaseStringUTFChars(env, pinyin, pinyin_c);
    (*env)->ReleaseStringUTFChars(env, mandarin_pinyin, mandarin_c);
    return status == 0 ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT jboolean JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeUserDictBoost(
    JNIEnv* env,
    jobject thiz,
    jlong handle,
    jstring headword
) {
    (void)thiz;
    const char* headword_c = (*env)->GetStringUTFChars(env, headword, NULL);
    int status = gannyu_pipeline_user_dict_boost((void*)(intptr_t)handle, headword_c, NULL);
    (*env)->ReleaseStringUTFChars(env, headword, headword_c);
    return status == 0 ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT jboolean JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeUserDataClear(
    JNIEnv* env, jobject thiz, jlong handle, jint scope
) {
    (void)env;
    (void)thiz;
    return gannyu_pipeline_user_data_clear((void*)(intptr_t)handle, scope) == 0
        ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT void JNICALL
Java_io_gannyu_input_GannyuInputMethodService_nativeDestroy(JNIEnv* env, jobject thiz, jlong handle) {
    (void)env;
    (void)thiz;
    gannyu_pipeline_destroy((void*)(intptr_t)handle);
}
