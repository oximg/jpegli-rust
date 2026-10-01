#include "shim.h"
#include <cstdio>
#include <cstdlib>
#include <limits>
#include <new>
#include "lib/jpegli/encode.h"

// All retained addresses live in a single calloc allocation. Every exception is
// caught before returning through the C ABI. No Rust callbacks or longjmp.
struct NativeFailure {};
struct oxj_encoder {
  jpeg_compress_struct cinfo;
  jpeg_error_mgr error;
  jpeg_destination_mgr dest;
  char message[JMSG_LENGTH_MAX];
  int status;
  uint64_t warnings;
  uint8_t* output;
  size_t capacity;
  size_t length;
  size_t max_output;
};

static oxj_encoder* context(j_common_ptr c) {
  return static_cast<oxj_encoder*>(c->client_data);
}

[[noreturn]] static void fail(oxj_encoder* e, int status, const char* text) {
  e->status = status;
  std::snprintf(e->message, sizeof(e->message), "%s", text);
  throw NativeFailure{};
}

static void error_exit(j_common_ptr c) {
  auto* e = context(c);
  e->status = OXJ_CODEC;
  c->err->format_message(c, e->message);
  throw NativeFailure{};
}

static void emit_message(j_common_ptr c, int level) {
  auto* e = context(c);
  if (level < 0 && e->warnings != UINT64_MAX) ++e->warnings;
}

static void init_destination(j_compress_ptr c) {
  auto* e = context(reinterpret_cast<j_common_ptr>(c));
  e->dest.next_output_byte = e->output;
  e->dest.free_in_buffer = e->capacity;
}

static boolean grow_destination(j_compress_ptr c) {
  auto* e = context(reinterpret_cast<j_common_ptr>(c));
  if (e->capacity == e->max_output)
    fail(e, OXJ_OUTPUT_LIMIT, "JPEG output exceeds max_output_bytes");
  size_t next = e->capacity > e->max_output / 2 ? e->max_output : e->capacity * 2;
  auto* data = static_cast<uint8_t*>(std::realloc(e->output, next));
  if (!data) fail(e, OXJ_ALLOC, "JPEG output allocation failed");
  e->output = data;
  e->dest.next_output_byte = data + e->capacity;
  e->dest.free_in_buffer = next - e->capacity;
  e->capacity = next;
  return TRUE;
}

static void term_destination(j_compress_ptr c) {
  auto* e = context(reinterpret_cast<j_common_ptr>(c));
  e->length = e->capacity - e->dest.free_in_buffer;
}

// Same spectrum-only script as oximg/oximg's jpegli_enc.rs (Apache-2.0).
static const jpeg_scan_info oximg_scans[] = {
  {3, {0, 1, 2, 0}, 0, 0, 0, 0},
  {1, {0, 0, 0, 0}, 1, 2, 0, 0},
  {1, {0, 0, 0, 0}, 3, 10, 0, 0},
  {1, {0, 0, 0, 0}, 11, 63, 0, 0},
  {1, {1, 0, 0, 0}, 1, 2, 0, 0},
  {1, {1, 0, 0, 0}, 3, 63, 0, 0},
  {1, {2, 0, 0, 0}, 1, 2, 0, 0},
  {1, {2, 0, 0, 0}, 3, 63, 0, 0},
};

// jpegli-static is deliberately built with exceptions enabled. Callback failures
// unwind C++ RAII objects, and are contained before the Rust caller resumes.
// This is not a guarantee of upstream exception safety under arbitrary OOM.
#define OXJ_CATCH \
  catch (const NativeFailure&) { return e->status; } \
  catch (const std::bad_alloc&) { \
    e->status = OXJ_ALLOC; \
    std::snprintf(e->message, sizeof(e->message), "native allocation failed"); \
    return e->status; \
  } catch (...) { \
    e->status = OXJ_CODEC; \
    std::snprintf(e->message, sizeof(e->message), "unexpected native exception"); \
    return e->status; \
  }

extern "C" oxj_encoder* oxj_alloc() {
  return static_cast<oxj_encoder*>(std::calloc(1, sizeof(oxj_encoder)));
}

extern "C" int oxj_start(oxj_encoder* e, uint32_t width, uint32_t height,
                          int quality, int scans, int sampling, size_t max_output) {
  try {
    jpegli_std_error(&e->error);
    e->error.error_exit = error_exit;
    e->error.emit_message = emit_message;
    e->cinfo.err = &e->error;
    e->cinfo.client_data = e;
    jpegli_create_compress(&e->cinfo);
    e->cinfo.client_data = e;
    e->cinfo.image_width = width;
    e->cinfo.image_height = height;
    e->cinfo.input_components = 3;
    e->cinfo.in_color_space = JCS_RGB;
    jpegli_set_defaults(&e->cinfo);
    // Preserve oximg's force_baseline=false quality semantics.
    jpegli_set_quality(&e->cinfo, quality, FALSE);
    e->cinfo.comp_info[0].h_samp_factor = sampling == 0 ? 1 : 2;
    e->cinfo.comp_info[0].v_samp_factor = sampling == 2 ? 2 : 1;
    for (int i = 1; i < 3; ++i) {
      e->cinfo.comp_info[i].h_samp_factor = 1;
      e->cinfo.comp_info[i].v_samp_factor = 1;
    }
    jpegli_set_progressive_level(&e->cinfo, scans == 0 ? 0 : 2);
    if (scans == 2) {
      e->cinfo.scan_info = oximg_scans;
      e->cinfo.num_scans = 8;
    }
    e->max_output = max_output;
    e->capacity = max_output < 65536 ? max_output : 65536;
    e->output = static_cast<uint8_t*>(std::malloc(e->capacity));
    if (!e->output) fail(e, OXJ_ALLOC, "JPEG output allocation failed");
    e->dest.init_destination = init_destination;
    e->dest.empty_output_buffer = grow_destination;
    e->dest.term_destination = term_destination;
    e->cinfo.dest = &e->dest;
    jpegli_start_compress(&e->cinfo, TRUE);
    return OXJ_OK;
  } OXJ_CATCH
}

extern "C" int oxj_write(oxj_encoder* e, const uint8_t* pixels,
                          size_t stride, uint32_t rows) {
  try {
    while (rows) {
      JSAMPROW batch[16];
      const uint32_t count = rows < 16 ? rows : 16;
      for (uint32_t i = 0; i < count; ++i)
        batch[i] = const_cast<JSAMPROW>(pixels + i * stride);
      const auto done = jpegli_write_scanlines(&e->cinfo, batch, count);
      if (!done || done > count) fail(e, OXJ_CODEC, "unexpected scanline progress");
      rows -= done;
      // Do not form a pointer beyond the last minimally padded row.
      if (rows) pixels += done * stride;
    }
    return OXJ_OK;
  } OXJ_CATCH
}

extern "C" int oxj_icc(oxj_encoder* e, const uint8_t* data, uint32_t len) {
  try {
    jpegli_write_icc_profile(&e->cinfo, data, len);
    return OXJ_OK;
  } OXJ_CATCH
}

extern "C" int oxj_finish(oxj_encoder* e) {
  try {
    jpegli_finish_compress(&e->cinfo);
    return OXJ_OK;
  } OXJ_CATCH
}

extern "C" int oxj_marker(oxj_encoder* e, int marker, const uint8_t* data, uint32_t len) {
  try {
    jpegli_write_marker(&e->cinfo, marker, data, len);
    return OXJ_OK;
  } OXJ_CATCH
}

extern "C" const char* oxj_message(const oxj_encoder* e) { return e->message; }
extern "C" uint64_t oxj_warnings(const oxj_encoder* e) { return e->warnings; }
extern "C" uint8_t* oxj_take_output(oxj_encoder* e, size_t* len) {
  auto* data = e->output;
  *len = e->length;
  e->output = nullptr;
  return data;
}
extern "C" void oxj_destroy(oxj_encoder* e) {
  if (!e) return;
  // jpegli_destroy_compress accepts a zero-initialized/partially created object.
  jpegli_destroy_compress(&e->cinfo);
  std::free(e->output);
  std::free(e);
}
extern "C" void oxj_free_output(uint8_t* data) { std::free(data); }
