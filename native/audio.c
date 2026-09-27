#define MINIAUDIO_IMPLEMENTATION
#include <miniaudio.h>
#include "bridge.h"
static ma_engine engine;
static ma_sound sound;
static int initialized = 0, loaded = 0;
int l2d_audio_init(void) {
    if (initialized) return 1;
    initialized = ma_engine_init(NULL, &engine) == MA_SUCCESS;
    return initialized;
}
int l2d_audio_load(const char* path) {
    if (!initialized) return 0;
    if (loaded) ma_sound_uninit(&sound);
    loaded = ma_sound_init_from_file(&engine, path, MA_SOUND_FLAG_STREAM, NULL, NULL, &sound) == MA_SUCCESS;
    return loaded;
}
int l2d_audio_play(void) { return loaded && ma_sound_start(&sound) == MA_SUCCESS; }
void l2d_audio_stop(void) { if (loaded) { ma_sound_stop(&sound); ma_sound_seek_to_pcm_frame(&sound, 0); } }
double l2d_audio_time(void) { float t = 0; if (loaded) ma_sound_get_cursor_in_seconds(&sound, &t); return t; }
void l2d_audio_shutdown(void) {
    if (loaded) ma_sound_uninit(&sound);
    if (initialized) ma_engine_uninit(&engine);
    loaded = initialized = 0;
}
