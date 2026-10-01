#ifndef OXIMG_JPEGLI_SHIM_H
#define OXIMG_JPEGLI_SHIM_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct oxj_encoder oxj_encoder;
enum { OXJ_OK = 0, OXJ_CODEC = 1, OXJ_OUTPUT_LIMIT = 2, OXJ_ALLOC = 3 };
oxj_encoder* oxj_alloc(void);
int oxj_start(oxj_encoder*, uint32_t width, uint32_t height, int quality,
              int scans, int sampling, size_t max_output);
int oxj_write(oxj_encoder*, const uint8_t* pixels, size_t stride, uint32_t rows);
int oxj_icc(oxj_encoder*, const uint8_t* data, uint32_t len);
int oxj_marker(oxj_encoder*, int marker, const uint8_t* data, uint32_t len);
int oxj_finish(oxj_encoder*);
const char* oxj_message(const oxj_encoder*);
uint64_t oxj_warnings(const oxj_encoder*);
uint8_t* oxj_take_output(oxj_encoder*, size_t* len);
void oxj_destroy(oxj_encoder*);
void oxj_free_output(uint8_t*);
#ifdef __cplusplus
}
#endif
#endif
