#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef _WIN32
#define L2D_API __declspec(dllexport)
#else
#define L2D_API __attribute__((visibility("default")))
#endif
#ifdef __cplusplus
extern "C" {
#endif
L2D_API const char* l2d_last_error(void);
L2D_API int l2d_init(void);
L2D_API void l2d_shutdown(void);
L2D_API void* l2d_load_model(const char* path);
L2D_API void l2d_destroy_model(void* model);
L2D_API const char* l2d_model_info(void* model);
L2D_API int l2d_get_param_count(void* model);
L2D_API const char* l2d_get_param_ids(void* model);
L2D_API int l2d_set_parameter(void* model, const char* id, float value);
L2D_API int l2d_set_parameters(void* model, const float* values, int count);
L2D_API int l2d_update(void* model, float dt);
L2D_API int l2d_draw(void* model, unsigned fbo, const float* transform);
L2D_API int l2d_anchor(void* model, int drawable, float* result);
L2D_API int l2d_nearest(void* model, float x, float y);
L2D_API unsigned l2d_canvas_create(int width, int height);
L2D_API void l2d_canvas_destroy(unsigned fbo);
L2D_API unsigned l2d_canvas_texture(unsigned fbo);
L2D_API int l2d_canvas_clear(unsigned fbo, float r, float g, float b, float a);
L2D_API int l2d_read_rgba(unsigned fbo, unsigned char* bytes, size_t size);
L2D_API int l2d_capture_submit(unsigned fbo, int64_t frame);
L2D_API int l2d_capture_poll(unsigned char* bytes, size_t size, int64_t* frame);
L2D_API void l2d_capture_reset(void);
L2D_API unsigned l2d_texture_load(const char* path);
L2D_API unsigned l2d_texture_rgba(unsigned texture, int width, int height, const unsigned char* data);
L2D_API void l2d_texture_destroy(unsigned texture);
L2D_API int l2d_texture_size(unsigned texture, int* dimensions);
L2D_API int l2d_draw_texture(unsigned texture, unsigned fbo, float x, float y, float w, float h, float rotation);
L2D_API int l2d_diagnostic(unsigned fbo, float time);
L2D_API int l2d_ui_init(void* window);
L2D_API void l2d_ui_shutdown(void);
L2D_API const char* l2d_ui_frame(unsigned texture, int width, int height, const char* status);
L2D_API int l2d_ui_canvas_point(double x, double y, float* result);
L2D_API int l2d_ui_snapshot(const char* path);
L2D_API int l2d_audio_init(void);
L2D_API int l2d_audio_load(const char* path);
L2D_API int l2d_audio_play(void);
L2D_API void l2d_audio_stop(void);
L2D_API double l2d_audio_time(void);
L2D_API void l2d_audio_shutdown(void);
#ifdef __cplusplus
}
#endif
