/* Minimal native PipeWire capture client. The RT callback never allocates or blocks. */
#include <math.h>
#include <pipewire/pipewire.h>
#include <signal.h>
#include <spa/param/audio/format-utils.h>
#include <stdatomic.h>
#include <stdio.h>

static struct pw_main_loop *loop;
static _Atomic float peak;

static void on_process(void *data) {
    struct pw_stream *stream = data;
    struct pw_buffer *pwb = pw_stream_dequeue_buffer(stream);
    if (!pwb) return;
    struct spa_buffer *b = pwb->buffer;
    if (b->n_datas && b->datas[0].data && b->datas[0].chunk) {
        const struct spa_chunk *chunk = b->datas[0].chunk;
        const float *samples = SPA_PTROFF(b->datas[0].data, chunk->offset, float);
        size_t count = chunk->size / sizeof(float);
        float value = 0.0f;
        for (size_t i = 0; i < count; ++i) value = fmaxf(value, fabsf(samples[i]));
        atomic_store_explicit(&peak, value, memory_order_relaxed);
    }
    pw_stream_queue_buffer(stream, pwb);
}

static const struct pw_stream_events stream_events = {
    PW_VERSION_STREAM_EVENTS, .process = on_process,
};

static void stop(int sig) { (void)sig; pw_main_loop_quit(loop); }

int main(int argc, char **argv) {
    pw_init(&argc, &argv);
    signal(SIGINT, stop); signal(SIGTERM, stop);
    loop = pw_main_loop_new(NULL);
    if (!loop) return 1;
    struct pw_context *context = pw_context_new(pw_main_loop_get_loop(loop), NULL, 0);
    struct pw_core *core = context ? pw_context_connect(context, NULL, 0) : NULL;
    struct pw_properties *props = pw_properties_new(
        PW_KEY_MEDIA_TYPE, "Audio", PW_KEY_MEDIA_CATEGORY, "Capture",
        PW_KEY_MEDIA_ROLE, "Production", PW_KEY_NODE_NAME, "rpi-vj-capture",
        PW_KEY_NODE_LATENCY, "256/48000", PW_KEY_NODE_FORCE_QUANTUM, "256", NULL);
    struct pw_stream *stream = core ? pw_stream_new(core, "rpi-vj line-in", props) : NULL;
    if (!stream) { fprintf(stderr, "unable to create PipeWire stream\n"); return 2; }
    struct spa_hook listener;
    pw_stream_add_listener(stream, &listener, &stream_events, stream);
    uint8_t storage[1024];
    struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(storage, sizeof(storage));
    struct spa_audio_info_raw info = SPA_AUDIO_INFO_RAW_INIT(
        .format = SPA_AUDIO_FORMAT_F32,
        .rate = 48000,
        .channels = 2,
        .position = { SPA_AUDIO_CHANNEL_FL, SPA_AUDIO_CHANNEL_FR });
    const struct spa_pod *params[] = { spa_format_audio_raw_build(&builder, SPA_PARAM_EnumFormat, &info) };
    int result = pw_stream_connect(stream, PW_DIRECTION_INPUT, PW_ID_ANY,
        PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS | PW_STREAM_FLAG_RT_PROCESS,
        params, 1);
    if (result < 0) { fprintf(stderr, "PipeWire connect failed: %s\n", spa_strerror(result)); return 3; }
    result = pw_main_loop_run(loop);
    pw_stream_destroy(stream); pw_core_disconnect(core); pw_context_destroy(context);
    pw_main_loop_destroy(loop); pw_deinit();
    return result < 0 ? 4 : 0;
}

