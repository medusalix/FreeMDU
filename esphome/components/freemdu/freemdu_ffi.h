// C interface to the FreeMDU protocol crate, implemented in ../../ffi/src/lib.rs.
// Keep the two in step: there is no generator, the declarations are written by hand.

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define FMDU_OK 0
#define FMDU_ERR_TIMEOUT -1
#define FMDU_ERR_PORT -2
#define FMDU_ERR_UNKNOWN_DEVICE -3
#define FMDU_ERR_PROTOCOL -4
#define FMDU_ERR_ARGUMENT -5
#define FMDU_ERR_DEVICE -6
#define FMDU_ERR_OTHER -7
#define FMDU_ERR_NO_ECHO -8
#define FMDU_ERR_UNAVAILABLE -9

#define FMDU_TEXT_MAX 64
// Request size of fmdu_read_memory(); its length must be a multiple of this.
#define FMDU_MEMORY_CHUNK 16

#define FMDU_KIND_GENERAL 0
#define FMDU_KIND_FAULT 1
#define FMDU_KIND_OPERATION 2
#define FMDU_KIND_IO 3

#define FMDU_VALUE_BOOL 0
#define FMDU_VALUE_NUMBER 1
#define FMDU_VALUE_SENSOR 2
#define FMDU_VALUE_STRING 3
#define FMDU_VALUE_DURATION 4
#define FMDU_VALUE_DATE 5
#define FMDU_VALUE_FAULT 6

// Reads up to len bytes, waiting at most timeout_ms for the first one.
// Returns the number of bytes read, 0 on timeout, negative on error.
typedef int32_t (*fmdu_read_fn)(void *ctx, uint8_t *buf, size_t len, uint32_t timeout_ms);
// Writes len bytes. Returns the number written, negative on error.
typedef int32_t (*fmdu_write_fn)(void *ctx, const uint8_t *buf, size_t len);
// level: 1 error, 2 warn, 3 info, 4 debug, 5 trace. msg is not NUL-terminated.
typedef void (*fmdu_log_fn)(uint8_t level, const char *msg, size_t len);

typedef struct {
  void *ctx;
  fmdu_read_fn read;
  fmdu_write_fn write;
  uint32_t timeout_ms;
  // Read back and discard every written byte (optical interface).
  bool discard_echo;
} fmdu_port_t;

// Strings are not NUL-terminated and live as long as the program.
typedef struct {
  const char *id;
  size_t id_len;
  const char *name;
  size_t name_len;
  const char *unit;  // NULL if none
  size_t unit_len;
  uint8_t kind;  // FMDU_KIND_*
} fmdu_property_t;

// kind      number                    target           count
// BOOL      0 or 1
// NUMBER    the number
// SENSOR    current value             target value
// STRING
// DURATION  seconds
// DATE      YYYYMMDD
// FAULT     0 ok, 1 active, 2 stored  operating hours  occurrences
// text always holds a NUL-terminated, human-readable form; name the implementation's display name.
typedef struct {
  uint8_t kind;  // FMDU_VALUE_*
  uint32_t number;
  uint32_t target;
  uint32_t count;
  char text[FMDU_TEXT_MAX];
  // Display name from the device implementation ("Running" for ProgramRunning); empty if none.
  char name[FMDU_TEXT_MAX];
} fmdu_value_t;

typedef struct fmdu_device_t fmdu_device_t;

// Queries the software ID and unlocks the device with the matching implementation.
// *out_software_id is set whenever the device answered, also for FMDU_ERR_UNKNOWN_DEVICE.
// The port's callbacks must stay usable until fmdu_free().
int32_t fmdu_connect(const fmdu_port_t *port, fmdu_device_t **out_dev, uint16_t *out_software_id);
void fmdu_free(fmdu_device_t *dev);

uint16_t fmdu_software_id(const fmdu_device_t *dev);
void fmdu_device_kind(const fmdu_device_t *dev, char *buf, size_t len);

size_t fmdu_property_count(const fmdu_device_t *dev);
bool fmdu_property_get(const fmdu_device_t *dev, size_t idx, fmdu_property_t *out);
// Returns -1 if there is no property with that ID.
int32_t fmdu_property_find(const fmdu_device_t *dev, const char *id, size_t len);
// FMDU_ERR_UNAVAILABLE: the property has no value in the current state; nothing is logged.
int32_t fmdu_query_property(fmdu_device_t *dev, size_t idx, fmdu_value_t *out);

// Reads len bytes of device memory from addr, with the access the device implementation unlocked.
int32_t fmdu_read_memory(fmdu_device_t *dev, uint32_t addr, uint8_t *buf, size_t len);

const char *fmdu_status_str(int32_t status);
// Not thread-safe: call once, before anything else.
void fmdu_set_logger(fmdu_log_fn func, uint8_t max_level);

#ifdef __cplusplus
}
#endif
