#pragma once

#include <atomic>
#include <string>
#include <vector>

#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <freertos/task.h>

#include "esphome/components/uart/uart.h"
#include "esphome/core/component.h"
#include "esphome/core/defines.h"
#include "esphome/core/preferences.h"

#ifdef USE_SENSOR
#include "esphome/components/sensor/sensor.h"
#endif
#ifdef USE_TEXT_SENSOR
#include "esphome/components/text_sensor/text_sensor.h"
#endif
#ifdef USE_BINARY_SENSOR
#include "esphome/components/binary_sensor/binary_sensor.h"
#endif
#ifdef USE_BUTTON
#include "esphome/components/button/button.h"
#include "esphome/core/helpers.h"
#endif

#include "freemdu_ffi.h"

namespace esphome::freemdu {

enum class TextInfo : uint8_t { SOFTWARE_ID, DEVICE_KIND, STATUS, ADAPTER_TEST };
enum class TextFormat : uint8_t { CODE, TEXT };

/// Turns the identifiers the device implementations use into English words:
/// "QuickWash40" -> "Quick wash 40", "Soak | PreWash" -> "Soak, pre wash".
std::string humanize(const char *code);

/// One entity bound to a device property.
struct PropertyBinding {
  std::string property;
#ifdef USE_SENSOR
  sensor::Sensor *sensor{nullptr};
  bool target{false};
#endif
#ifdef USE_TEXT_SENSOR
  text_sensor::TextSensor *text_sensor{nullptr};
  TextFormat format{TextFormat::CODE};
#endif
#ifdef USE_BINARY_SENSOR
  binary_sensor::BinarySensor *binary_sensor{nullptr};
#endif
};

/// Outcome of one poll, handed from the worker task to the main loop.
struct PollResult {
  int32_t status{FMDU_ERR_OTHER};
  uint16_t software_id{0};
  std::string device_kind;
  /// Parallel to bindings_; `ok` is false for a property that failed or does not exist.
  std::vector<bool> ok;
  std::vector<fmdu_value_t> values;
  /// Memory read for memory_log; empty if disabled or the read failed.
  std::vector<uint8_t> memory;
};

/// Polls a Miele appliance over its diagnostic interface.
///
/// The protocol runs at 2400 baud and must acknowledge each received chunk promptly, so a poll
/// (seconds) runs in a FreeRTOS task of its own. Results are published from loop().
class FreeMDUComponent : public PollingComponent, public uart::UARTDevice {
 public:
  void setup() override;
  void update() override;
  void loop() override;
  void dump_config() override;
  float get_setup_priority() const override { return setup_priority::DATA; }

  void set_discard_echo(bool discard_echo) { this->discard_echo_ = discard_echo; }
  void set_timeout(uint32_t timeout_ms) { this->timeout_ms_ = timeout_ms; }
  void set_protocol_log(bool protocol_log) { this->protocol_log_ = protocol_log; }
  void set_hold_time(uint32_t hold_time_ms) { this->hold_time_ms_ = hold_time_ms; }
  void set_reconnect_interval(uint32_t reconnect_interval_ms) { this->reconnect_interval_ms_ = reconnect_interval_ms; }
  /// Starts the adapter test: a minute of echo checks with polling paused, which needs no
  /// appliance. Not run while the appliance answers, as that already proves the adapter works.
  void start_adapter_test();
  bool is_testing() const { return this->testing_; }
  void set_memory_log(uint32_t address, uint32_t length) {
    this->memory_log_address_ = address;
    this->memory_log_length_ = length;
  }

#ifdef USE_SENSOR
  void add_sensor(const char *property, sensor::Sensor *sensor, bool target);

  /// Enables the energy estimate. All powers in watts. Without a heater power, the device's own
  /// estimated_power property is used instead of the model.
  void set_energy_model(float heater_power, float running_power, float standby_power, float drum_power_per_1000_rpm);
  void set_power_sensor(sensor::Sensor *sensor) { this->power_sensor_ = sensor; }
  void set_energy_sensor(sensor::Sensor *sensor) { this->energy_sensor_ = sensor; }
  void set_program_energy_sensor(sensor::Sensor *sensor) { this->program_energy_sensor_ = sensor; }
#endif
#ifdef USE_TEXT_SENSOR
  void add_text_sensor(const char *property, text_sensor::TextSensor *text_sensor,
                       TextFormat format = TextFormat::CODE);
  void set_info_text_sensor(TextInfo info, text_sensor::TextSensor *text_sensor);
#endif
#ifdef USE_BINARY_SENSOR
  void add_binary_sensor(const char *property, binary_sensor::BinarySensor *binary_sensor);
  void set_connected_binary_sensor(binary_sensor::BinarySensor *sensor) { this->connected_sensor_ = sensor; }
  void set_echo_binary_sensor(binary_sensor::BinarySensor *sensor) { this->echo_sensor_ = sensor; }
#endif

 protected:
  static void task_entry_(void *arg);
  static int32_t port_read_(void *ctx, uint8_t *buf, size_t len, uint32_t timeout_ms);
  static int32_t port_write_(void *ctx, const uint8_t *buf, size_t len);

  /// Runs in the worker task.
  void poll_(PollResult &result);
  void log_memory_(const std::vector<uint8_t> &memory);
  void publish_(const PollResult &result);
  void publish_unknown_();
  /// Runs in the worker task.
  void run_adapter_test_();
  void publish_adapter_test_(const char *text);
  /// Logs the properties whose value changed since the last poll.
  void log_changes_(const PollResult &result);
  /// Returns the index of the binding for a property, adding one without entities if needed.
  size_t require_property_(const char *property);
#ifdef USE_SENSOR
  void setup_energy_();
  void update_energy_(const PollResult &result, bool connected);
#endif

  std::vector<PropertyBinding> bindings_;
  bool discard_echo_{true};
  uint32_t timeout_ms_{500};
  bool protocol_log_{false};
  uint32_t hold_time_ms_{30000};
  /// Poll interval while the appliance does not answer; 0 keeps update_interval.
  uint32_t reconnect_interval_ms_{1000};
  /// When update() last started a poll.
  uint32_t last_attempt_{0};
  uint32_t memory_log_address_{0};
  /// 0 disables the memory log.
  uint32_t memory_log_length_{0};

  TaskHandle_t task_{nullptr};
  SemaphoreHandle_t lock_{nullptr};
  std::atomic<bool> busy_{false};
  /// Guarded by lock_.
  PollResult pending_;
  std::atomic<bool> has_pending_{false};
  /// Lowest free stack seen in the worker task, in bytes. Worker task only.
  UBaseType_t stack_headroom_{UINT32_MAX};

  bool connected_{false};
  /// Whether the entities still show values from before the current run of failed polls.
  bool holding_{false};
  /// When the current run of failed polls began.
  uint32_t failing_since_{0};
  bool first_publish_{true};
  int32_t last_status_{FMDU_OK};
  /// Text of each property as last logged, parallel to bindings_; empty until logged.
  std::vector<std::string> logged_;
  /// Properties already reported as missing for this software ID.
  uint16_t warned_software_id_{0};

  std::atomic<bool> testing_{false};
  std::atomic<bool> test_requested_{false};
  /// Echo of the latest test burst: -1 no test running, 0 missing, 1 all bytes back. Set by the worker task.
  std::atomic<int8_t> test_echo_{-1};
  int8_t published_echo_{-2};
  /// Outcome of the finished test. Guarded by lock_.
  std::string test_result_;
  std::atomic<bool> has_test_result_{false};

#ifdef USE_TEXT_SENSOR
  text_sensor::TextSensor *software_id_sensor_{nullptr};
  text_sensor::TextSensor *device_kind_sensor_{nullptr};
  text_sensor::TextSensor *status_sensor_{nullptr};
  text_sensor::TextSensor *adapter_test_sensor_{nullptr};
#endif
#ifdef USE_BINARY_SENSOR
  binary_sensor::BinarySensor *connected_sensor_{nullptr};
  binary_sensor::BinarySensor *echo_sensor_{nullptr};
#endif

#ifdef USE_SENSOR
  /// Persisted across reboots, so that the total keeps increasing and a program interrupted
  /// by a reboot is still measured from its start.
  struct EnergyState {
    float total_kwh;
    float program_start_kwh;
    bool in_program;
    /// NaN until a program has finished.
    float last_program_kwh;
  };
  /// Layout saved before last_program_kwh existed, read once to migrate.
  struct EnergyStateV0 {
    float total_kwh;
    float program_start_kwh;
    bool in_program;
  };

  bool energy_enabled_{false};
  float heater_power_{0.0f};
  float running_power_{0.0f};
  float standby_power_{0.0f};
  float drum_power_per_1000_rpm_{0.0f};
  /// Whether the missing operating state has been reported for program_energy.
  bool program_energy_warned_{false};
  /// Bindings the estimate reads, -1 until setup.
  int actuators_index_{-1};
  int state_index_{-1};
  int drum_index_{-1};
  int power_index_{-1};
  /// Whether a device without estimated_power has been reported.
  bool device_power_warned_{false};
  /// Power since the last poll, in watts, and when it was set.
  float power_{0.0f};
  uint32_t power_since_{0};
  EnergyState energy_{0.0f, 0.0f, false, NAN};
  float saved_total_kwh_{0.0f};
  ESPPreferenceObject energy_pref_;
  sensor::Sensor *power_sensor_{nullptr};
  sensor::Sensor *energy_sensor_{nullptr};
  sensor::Sensor *program_energy_sensor_{nullptr};
#endif
};

#ifdef USE_BUTTON
/// Starts the adapter test.
class AdapterTestButton : public button::Button, public Parented<FreeMDUComponent> {
 protected:
  void press_action() override { this->parent_->start_adapter_test(); }
};
#endif

}  // namespace esphome::freemdu
