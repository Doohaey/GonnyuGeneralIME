#include "gannyu_input.h"
#include <ibus.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <wchar.h>
#include <locale.h>

#define GANNYU_TYPE_ENGINE (gannyu_engine_get_type())
#define GANNYU_ENGINE(obj) (G_TYPE_CHECK_INSTANCE_CAST((obj), GANNYU_TYPE_ENGINE, GannyuEngine))

typedef struct _GannyuEngine GannyuEngine;
typedef struct _GannyuEngineClass GannyuEngineClass;

struct _GannyuEngine {
    IBusEngine parent;
    GString *buffer;
    IBusLookupTable *table;
    GannyuPipelineHandle *pipeline;
    char **candidate_texts;
    guint candidate_count;
};

struct _GannyuEngineClass {
    IBusEngineClass parent;
};

static GType gannyu_engine_get_type(void);
static void gannyu_engine_class_init(GannyuEngineClass *klass);
static void gannyu_engine_init(GannyuEngine *engine);
static void gannyu_engine_destroy(IBusObject *object);
static gboolean gannyu_engine_process_key_event(IBusEngine *engine, guint keyval, guint keycode, guint state);
static void gannyu_engine_reset(GannyuEngine *self);
static void gannyu_engine_refresh(GannyuEngine *self);
static void gannyu_engine_commit_index(GannyuEngine *self, guint index);
static void gannyu_engine_ensure_runtime_ready(GannyuEngine *self);

G_DEFINE_TYPE(GannyuEngine, gannyu_engine, IBUS_TYPE_ENGINE)

static char *g_manifest_path = NULL;
static char *g_region_id = NULL;

static char *resolve_manifest_path(void) {
    const char *env = g_getenv("GANNYU_MANIFEST");
    if (env && *env) return g_strdup(env);
    return g_strdup("");
}

static void gannyu_engine_class_init(GannyuEngineClass *klass) {
    IBusObjectClass *object_class = IBUS_OBJECT_CLASS(klass);
    IBusEngineClass *engine_class = IBUS_ENGINE_CLASS(klass);
    object_class->destroy = gannyu_engine_destroy;
    engine_class->process_key_event = gannyu_engine_process_key_event;
}

static void gannyu_engine_init(GannyuEngine *self) {
    self->buffer = g_string_new("");
    self->table = ibus_lookup_table_new(9, 0, TRUE, TRUE);
    g_object_ref_sink(self->table);
    self->pipeline = NULL;
    self->candidate_texts = NULL;
    self->candidate_count = 0;

    GannyuPipelineHandle *handle = NULL;
    int code = gannyu_pipeline_create(g_manifest_path, g_region_id, &handle);
    if (code == 0) self->pipeline = handle;
    else g_warning("gannyu pipeline_create failed code=%d manifest=%s region=%s",
                   code, g_manifest_path, g_region_id ? g_region_id : "(default)");
}

static void gannyu_engine_destroy(IBusObject *object) {
    GannyuEngine *self = GANNYU_ENGINE(object);
    if (self->pipeline) {
        gannyu_pipeline_destroy(self->pipeline);
        self->pipeline = NULL;
    }
    if (self->buffer) {
        g_string_free(self->buffer, TRUE);
        self->buffer = NULL;
    }
    if (self->table) {
        g_object_unref(self->table);
        self->table = NULL;
    }
    if (self->candidate_texts) {
        for (guint i = 0; i < self->candidate_count; ++i) g_free(self->candidate_texts[i]);
        g_free(self->candidate_texts);
        self->candidate_texts = NULL;
        self->candidate_count = 0;
    }
    IBUS_OBJECT_CLASS(gannyu_engine_parent_class)->destroy(object);
}

static gchar *extract_field(const char *json, const char *start_marker, char end_char, gsize *out_len) {
    const char *p = strstr(json, start_marker);
    if (!p) return NULL;
    p += strlen(start_marker);
    const char *e = strchr(p, end_char);
    if (!e) return NULL;
    *out_len = (gsize)(e - p);
    return g_strndup(p, *out_len);
}

static void parse_candidates(GannyuEngine *self, const char *json) {
    if (self->candidate_texts) {
        for (guint i = 0; i < self->candidate_count; ++i) g_free(self->candidate_texts[i]);
        g_free(self->candidate_texts);
        self->candidate_texts = NULL;
        self->candidate_count = 0;
    }
    ibus_lookup_table_clear(self->table);

    GPtrArray *items = g_ptr_array_new();
    const char *cursor = json;
    while ((cursor = strstr(cursor, "\"text\":\"")) != NULL) {
        cursor += strlen("\"text\":\"");
        const char *end = cursor;
        while (*end && !(*end == '"' && *(end - 1) != '\\')) end++;
        if (*end != '"') break;
        gchar *text = g_strndup(cursor, (gsize)(end - cursor));
        g_ptr_array_add(items, text);
        cursor = end + 1;
    }

    self->candidate_count = items->len;
    self->candidate_texts = (char **)g_malloc0(sizeof(char *) * items->len);
    for (guint i = 0; i < items->len; ++i) {
        gchar *t = (gchar *)g_ptr_array_index(items, i);
        self->candidate_texts[i] = t;
        IBusText *itext = ibus_text_new_from_string(t);
        ibus_lookup_table_append_candidate(self->table, itext);
    }
    g_ptr_array_free(items, FALSE);
}

static void gannyu_engine_ensure_runtime_ready(GannyuEngine *self) {
    /* Auto-started input methods can come up before a late-mounted resource
     * directory is available after reboot, so retry pipeline creation on use. */
    if (self->pipeline) return;
    GannyuPipelineHandle *handle = NULL;
    if (gannyu_pipeline_create(g_manifest_path, g_region_id, &handle) == 0) {
        self->pipeline = handle;
    }
}

/* Map common ASCII punctuation to Chinese full-width equivalents.
 * Returns a static UTF-8 string, or NULL if no mapping exists. */
static const char *chinese_fullwidth_symbol(guint keyval) {
    switch (keyval) {
        case ',':  return "\xEF\xBC\x8C"; /* ，U+FF0C */
        case '.':  return "\xE3\x80\x82"; /* 。U+3002 */
        case '\\': return "\xE3\x80\x81"; /* 、U+3001 */
        case ';':  return "\xEF\xBC\x9B"; /* ；U+FF1B */
        case ':':  return "\xEF\xBC\x9A"; /* ：U+FF1A */
        case '?':  return "\xEF\xBC\x9F"; /* ？U+FF1F */
        case '!':  return "\xEF\xBC\x81"; /* ！U+FF01 */
        case '(':  return "\xEF\xBC\x88"; /* （U+FF08 */
        case ')':  return "\xEF\xBC\x89"; /* ）U+FF09 */
        case '[':  return "\xE3\x80\x90"; /* 【U+3010 */
        case ']':  return "\xE3\x80\x91"; /* 】U+3011 */
        case '<':  return "\xE3\x80\x8A"; /* 《U+300A */
        case '>':  return "\xE3\x80\x8B"; /* 》U+300B */
        case '"':  return "\xE2\x80\x9C"; /* "U+201C left double quote */
        case '~':  return "\xEF\xBD\x9E"; /* ～U+FF5E */
        case '-':  return "\xEF\xBC\x8D"; /* －U+FF0D */
        default:   return NULL;
    }
}

static void gannyu_engine_refresh(GannyuEngine *self) {
    IBusEngine *engine = IBUS_ENGINE(self);
    gannyu_engine_ensure_runtime_ready(self);
    if (self->buffer->len == 0) {
        ibus_engine_hide_lookup_table(engine);
        ibus_engine_hide_preedit_text(engine);
        return;
    }
    IBusText *preedit = ibus_text_new_from_string(self->buffer->str);
    ibus_engine_update_preedit_text(engine, preedit, self->buffer->len, TRUE);

    if (!self->pipeline) return;
    char *json = NULL;
    int code = gannyu_pipeline_compose(self->pipeline, self->buffer->str, &json);
    if (code != 0 || !json) {
        ibus_engine_hide_lookup_table(engine);
        return;
    }
    parse_candidates(self, json);
    gannyu_string_destroy(json);
    if (self->candidate_count > 0)
        ibus_engine_update_lookup_table(engine, self->table, TRUE);
    else
        ibus_engine_hide_lookup_table(engine);
}

static void gannyu_engine_commit_index(GannyuEngine *self, guint index) {
    if (index >= self->candidate_count) return;
    IBusEngine *engine = IBUS_ENGINE(self);
    IBusText *text = ibus_text_new_from_string(self->candidate_texts[index]);
    ibus_engine_commit_text(engine, text);
    gannyu_engine_reset(self);
}

static void gannyu_engine_reset(GannyuEngine *self) {
    g_string_truncate(self->buffer, 0);
    IBusEngine *engine = IBUS_ENGINE(self);
    ibus_engine_hide_preedit_text(engine);
    ibus_engine_hide_lookup_table(engine);
}

static gboolean gannyu_engine_process_key_event(IBusEngine *engine, guint keyval, guint keycode, guint state) {
    (void)keycode;
    if (state & IBUS_RELEASE_MASK) return FALSE;
    if (state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_SUPER_MASK)) return FALSE;
    GannyuEngine *self = GANNYU_ENGINE(engine);
    gannyu_engine_ensure_runtime_ready(self);

    if (keyval >= IBUS_KEY_1 && keyval <= IBUS_KEY_9 && self->candidate_count > 0) {
        gannyu_engine_commit_index(self, keyval - IBUS_KEY_1);
        return TRUE;
    }
    /* Pass through number keys (0-9) when no candidates are showing */
    if (keyval >= IBUS_KEY_0 && keyval <= IBUS_KEY_9 && self->candidate_count == 0 && self->buffer->len == 0) {
        return FALSE;
    }
    if (keyval == IBUS_KEY_space) {
        if (self->candidate_count > 0) { gannyu_engine_commit_index(self, 0); return TRUE; }
        if (self->buffer->len > 0) {
            IBusText *raw = ibus_text_new_from_string(self->buffer->str);
            ibus_engine_commit_text(engine, raw);
            gannyu_engine_reset(self);
            return TRUE;
        }
        return FALSE;
    }
    if (keyval == IBUS_KEY_Return) {
        if (self->buffer->len > 0) {
            IBusText *raw = ibus_text_new_from_string(self->buffer->str);
            ibus_engine_commit_text(engine, raw);
            gannyu_engine_reset(self);
            return TRUE;
        }
        return FALSE;
    }
    if (keyval == IBUS_KEY_BackSpace) {
        if (self->buffer->len > 0) {
            g_string_truncate(self->buffer, self->buffer->len - 1);
            gannyu_engine_refresh(self);
            return TRUE;
        }
        return FALSE;
    }
    if (keyval == IBUS_KEY_Escape) {
        if (self->buffer->len > 0) { gannyu_engine_reset(self); return TRUE; }
        return FALSE;
    }
    /* Convert common ASCII punctuation to Chinese full-width when buffer is empty */
    if (self->buffer->len == 0) {
        const char *fw = chinese_fullwidth_symbol(keyval);
        if (fw) {
            IBusText *sym = ibus_text_new_from_string(fw);
            ibus_engine_commit_text(engine, sym);
            return TRUE;
        }
    }
    if (keyval >= 0x21 && keyval < 0x7F) {
        g_string_append_c(self->buffer, (gchar)keyval);
        gannyu_engine_refresh(self);
        return TRUE;
    }
    return FALSE;
}

int main(int argc, char *argv[]) {
    gboolean ibus_mode = FALSE;
    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--ibus") == 0) ibus_mode = TRUE;
    }

    ibus_init();
    g_manifest_path = resolve_manifest_path();
    g_region_id = (char *)g_getenv("GANNYU_REGION_ID");

    IBusBus *bus = ibus_bus_new();
    if (!ibus_bus_is_connected(bus)) {
        fprintf(stderr, "ibus bus 未连接\n");
        return EXIT_FAILURE;
    }
    g_signal_connect(bus, "disconnected", G_CALLBACK(ibus_quit), NULL);

    IBusFactory *factory = ibus_factory_new(ibus_bus_get_connection(bus));
    g_object_ref_sink(factory);
    ibus_factory_add_engine(factory, "gannyu-gannyu", GANNYU_TYPE_ENGINE);

    if (ibus_mode) {
        ibus_bus_request_name(bus, "org.gannyu.input.gannyu", 0);
    } else {
        if (!ibus_bus_register_component(bus, ibus_component_new(
                "org.gannyu.input.gannyu", "Gannyu Gannyu", GONNYU_VERSION,
                "Apache-2.0", "Gannyu", "https://github.com/", "", "ibus-gannyu"))) {
            fprintf(stderr, "register_component 失败\n");
        }
    }

    ibus_main();
    g_object_unref(factory);
    g_object_unref(bus);
    g_free(g_manifest_path);
    return EXIT_SUCCESS;
}
