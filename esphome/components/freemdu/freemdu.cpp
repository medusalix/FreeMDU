#include "freemdu.h"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cctype>
#include <cstring>

#include "esphome/core/hal.h"
#include "esphome/core/helpers.h"
#include "esphome/core/log.h"

#ifdef USE_CONTROLLER_REGISTRY
#include "esphome/core/controller_registry.h"
#endif

namespace esphome::freemdu {

static const char *const TAG = "freemdu";

// Stack size in bytes. The protocol futures are polled on this stack; 8 KiB was not enough
// headroom for the deeper device implementations during development.
static constexpr uint32_t TASK_STACK_SIZE = 12 * 1024;
// Above the main loop (1), so that chunks are acknowledged in time while the loop is busy.
// The task sleeps whenever it waits for the UART.
static constexpr UBaseType_t TASK_PRIORITY = 2;

std::string humanize(const char *code) {
  std::string out;
  bool start_of_value = true;

  for (const char *p = code; *p != '\0'; p++) {
    const char c = *p;

    // Flag sets are joined with " | "
    if (c == ' ' && p[1] == '|' && p[2] == ' ') {
      out += ", ";
      p += 2;
      continue;
    }

    if (!out.empty() && out.back() != ' ') {
      const char prev = p[-1];
      const bool upper = isupper(static_cast<unsigned char>(c));
      const bool digit = isdigit(static_cast<unsigned char>(c));
      const bool prev_lower = islower(static_cast<unsigned char>(prev));
      const bool prev_digit = isdigit(static_cast<unsigned char>(prev));
      const bool prev_upper = isupper(static_cast<unsigned char>(prev));
      const bool next_lower = islower(static_cast<unsigned char>(p[1]));
      // A word starts at an upper-case letter after a lower-case letter or digit, at the last
      // capital of an acronym followed by lower case ("NTCValue" -> "NTC value"), and at a
      // digit after a letter
      if ((upper && (prev_lower || prev_digit)) || (upper && prev_upper && next_lower) ||
          (digit && !prev_digit && !isspace(static_cast<unsigned char>(prev))))
        out += ' ';
    }

    // Keep acronyms (two capitals in a row) and the first letter of each value as they are
    const bool acronym = isupper(static_cast<unsigned char>(c)) &&
                         (isupper(static_cast<unsigned char>(p[1])) ||
                          (p != code && isupper(static_cast<unsigned char>(p[-1])) && !islower(static_cast<unsigned char>(p[1]))));
    out += (start_of_value || acronym) ? c : static_cast<char>(tolower(static_cast<unsigned char>(c)));
    start_of_value = false;
  }

  return out;
}

// Last warning from the library. A failing connection repeats the same warning at every attempt,
// so it is logged once until something else happens. Worker task only, like every library call.
static std::string last_library_warning;

// Set from protocol_log. The library's debug and trace then log at DEBUG, the logger's default
// level, which would otherwise compile them out and leave protocol_log showing nothing.
static bool library_trace = false;

static void log_from_library(uint8_t level, const char *msg, size_t len) {
  if (level <= 2) {
    if (last_library_warning.size() == len && last_library_warning.compare(0, len, msg, len) == 0) {
      ESP_LOGV(TAG, "%.*s", static_cast<int>(len), msg);
      return;
    }
    last_library_warning.assign(msg, len);
  }
  if (level >= 4 && library_trace)
    level = 4;

  switch (level) {
    case 1:
      ESP_LOGE(TAG, "%.*s", static_cast<int>(len), msg);
      break;
    case 2:
      ESP_LOGW(TAG, "%.*s", static_cast<int>(len), msg);
      break;
    case 3:
      ESP_LOGI(TAG, "%.*s", static_cast<int>(len), msg);
      break;
    case 4:
      ESP_LOGD(TAG, "%.*s", static_cast<int>(len), msg);
      break;
    default:
      ESP_LOGV(TAG, "%.*s", static_cast<int>(len), msg);
      break;
  }
}

void FreeMDUComponent::setup() {
#ifdef USE_SENSOR
  // Before the worker task starts: this may add bindings
  this->setup_energy_();
#endif

  this->lock_ = xSemaphoreCreateMutex();
  // Warnings carry the reason a poll failed; trace shows every byte
  library_trace = this->protocol_log_;
  fmdu_set_logger(log_from_library, this->protocol_log_ ? 5 : 2);

  if (this->lock_ == nullptr ||
      xTaskCreate(task_entry_, "freemdu", TASK_STACK_SIZE, this, TASK_PRIORITY, &this->task_) != pdPASS) {
    ESP_LOGE(TAG, "Failed to start the worker task");
    this->mark_failed();
  }
}

void FreeMDUComponent::update() {
  if (this->task_ == nullptr || this->testing_)
    return;

  if (this->busy_.exchange(true)) {
    // Expected while reconnecting: an attempt started by loop() may still be waiting
    if (this->connected_)
      ESP_LOGD(TAG, "Previous poll still running, skipping");
    return;
  }

  this->last_attempt_ = millis();
  xTaskNotifyGive(this->task_);
}

void FreeMDUComponent::task_entry_(void *arg) {
  auto *self = static_cast<FreeMDUComponent *>(arg);
  PollResult result;

  for (;;) {
    ulTaskNotifyTake(pdTRUE, portMAX_DELAY);
    if (self->test_requested_.exchange(false)) {
      self->run_adapter_test_();
      continue;
    }
    self->poll_(result);

    // Report when the stack headroom shrinks, to size TASK_STACK_SIZE from real devices
    const UBaseType_t headroom = uxTaskGetStackHighWaterMark(nullptr);
    if (headroom < self->stack_headroom_) {
      self->stack_headroom_ = headroom;
      ESP_LOGD(TAG, "Worker stack headroom: %u of %" PRIu32 " bytes", static_cast<unsigned>(headroom),
               TASK_STACK_SIZE);
    }

    xSemaphoreTake(self->lock_, portMAX_DELAY);
    std::swap(self->pending_, result);
    self->has_pending_ = true;
    xSemaphoreGive(self->lock_);

    self->busy_ = false;
  }
}

int32_t FreeMDUComponent::port_read_(void *ctx, uint8_t *buf, size_t len, uint32_t timeout_ms) {
  auto *self = static_cast<FreeMDUComponent *>(ctx);
  const uint32_t start = millis();
  size_t available;

  while ((available = self->available()) == 0) {
    if (millis() - start >= timeout_ms)
      return 0;
    vTaskDelay(1);
  }

  const size_t n = std::min(available, len);
  return self->read_array(buf, n) ? static_cast<int32_t>(n) : -1;
}

int32_t FreeMDUComponent::port_write_(void *ctx, const uint8_t *buf, size_t len) {
  auto *self = static_cast<FreeMDUComponent *>(ctx);

  self->write_array(buf, len);
  return static_cast<int32_t>(len);
}

void FreeMDUComponent::poll_(PollResult &result) {
  const size_t count = this->bindings_.size();

  result.status = FMDU_ERR_OTHER;
  result.software_id = 0;
  result.device_kind.clear();
  result.ok.assign(count, false);
  result.values.resize(count);

  // Drop anything left in the receive buffer, e.g. a late answer to a failed poll
  uint8_t byte;
  while (this->available() > 0 && this->read_byte(&byte)) {
  }

  fmdu_port_t port{this, port_read_, port_write_, this->timeout_ms_, this->discard_echo_};
  fmdu_device_t *dev = nullptr;

  result.status = fmdu_connect(&port, &dev, &result.software_id);
  if (result.status != FMDU_OK)
    return;
  last_library_warning.clear();

  char kind[FMDU_TEXT_MAX];
  fmdu_device_kind(dev, kind, sizeof(kind));
  result.device_kind = kind;

  const bool warn_missing = result.software_id != this->warned_software_id_;
  this->warned_software_id_ = result.software_id;

  for (size_t i = 0; i < count; i++) {
    const std::string &property = this->bindings_[i].property;

    // Several entities may show the same property; query it once
    auto earlier = std::find_if(this->bindings_.begin(), this->bindings_.begin() + i,
                                [&](const PropertyBinding &binding) { return binding.property == property; });
    if (earlier != this->bindings_.begin() + i) {
      const size_t j = earlier - this->bindings_.begin();
      result.ok[i] = result.ok[j];
      result.values[i] = result.values[j];
      continue;
    }

    const int32_t idx = fmdu_property_find(dev, property.data(), property.size());
    if (idx < 0) {
      if (warn_missing)
        ESP_LOGW(TAG, "Software ID %u has no property '%s'", result.software_id, property.c_str());
      continue;
    }

    const int32_t status = fmdu_query_property(dev, idx, &result.values[i]);
    if (status == FMDU_OK) {
      result.ok[i] = true;
    } else if (status == FMDU_ERR_TIMEOUT || status == FMDU_ERR_NO_ECHO || status == FMDU_ERR_PORT ||
               status == FMDU_ERR_PROTOCOL) {
      // The link is gone; the rest would only time out as well
      result.status = status;
      break;
    }
  }

  // Logged from the main loop: the logger keeps only a few lines from other tasks
  result.memory.clear();
  if (result.status == FMDU_OK && this->memory_log_length_ > 0) {
    result.memory.resize(this->memory_log_length_);
    const int32_t status = fmdu_read_memory(dev, this->memory_log_address_, result.memory.data(), result.memory.size());
    if (status != FMDU_OK) {
      ESP_LOGW(TAG, "Memory log failed: %s", fmdu_status_str(status));
      result.memory.clear();
    }
  }

  fmdu_free(dev);
}

void FreeMDUComponent::log_memory_(const std::vector<uint8_t> &memory) {
  // 32 bytes per line: "mem 0x00c0: 0011…" stays well inside the logger's line buffer
  static constexpr size_t LINE = 32;
  char hex[LINE * 2 + 1];

  for (size_t off = 0; off < memory.size(); off += LINE) {
    const size_t n = std::min(LINE, memory.size() - off);
    for (size_t i = 0; i < n; i++)
      snprintf(hex + i * 2, 3, "%02x", memory[off + i]);
    ESP_LOGI(TAG, "mem 0x%04" PRIx32 ": %s", static_cast<uint32_t>(this->memory_log_address_ + off), hex);
  }
}

void FreeMDUComponent::start_adapter_test() {
  if (this->task_ == nullptr || this->testing_)
    return;

  if (!this->discard_echo_) {
    this->publish_adapter_test_("Not available: needs the optical echo (discard_echo)");
    return;
  }
  if (this->connected_) {
    this->publish_adapter_test_("Not needed: the appliance answers, so the adapter works");
    return;
  }

  this->publish_adapter_test_("Running");
  this->testing_ = true;
  this->test_requested_ = true;
  // Picked up at once, or right after a poll that is still running
  xTaskNotifyGive(this->task_);
}

void FreeMDUComponent::run_adapter_test_() {
  static constexpr uint32_t DURATION_MS = 60000;
  static constexpr size_t BURST = 15;
  // No five of these bytes in a row add up to a request with a valid checksum, so an appliance
  // that sees the test cannot take any part of it for a command
  static constexpr uint8_t PATTERN[] = {0xFF, 0xF0, 0x0F, 0xAA, 0x55};

  uint8_t byte;
  auto drain = [&]() {
    size_t n = 0;
    while (this->available() > 0 && this->read_byte(&byte))
      n++;
    return n;
  };

  // A quiet line first: anything received now is light or noise, not echo
  drain();
  vTaskDelay(pdMS_TO_TICKS(300));
  const size_t noise = drain();
  ESP_LOGI(TAG, "Adapter test: %u bytes on a quiet line, now checking the echo for %" PRIu32 " s",
           static_cast<unsigned>(noise), DURATION_MS / 1000);

  // The library needs the echo's bytes, not their values, so a burst counts as echoed once all its
  // bytes are back; a weak reflection garbles some of them, which is reported but does no harm
  uint32_t bursts = 0, echoed = 0, exact = 0, silent = 0;
  bool shown_garbled = false, shown_short = false;
  uint8_t out[BURST], in[BURST];
  const uint32_t start = millis();
  uint32_t reported = start;

  while (millis() - start < DURATION_MS) {
    for (size_t i = 0; i < BURST; i++)
      out[i] = PATTERN[(i * 2 + bursts) % sizeof(PATTERN)];
    this->write_array(out, BURST);

    size_t got = 0;
    while (got < BURST) {
      const int32_t n = port_read_(this, in + got, BURST - got, 100);
      if (n <= 0)
        break;
      got += n;
    }

    bursts++;
    if (got == BURST) {
      echoed++;
      if (memcmp(in, out, BURST) == 0) {
        exact++;
      } else if (!shown_garbled) {
        shown_garbled = true;
        ESP_LOGD(TAG, "Adapter test: sent %s, got %s", format_hex_pretty(out, BURST).c_str(),
                 format_hex_pretty(in, BURST).c_str());
      }
    } else if (got == 0) {
      silent++;
    } else if (!shown_short) {
      shown_short = true;
      ESP_LOGD(TAG, "Adapter test: sent %s, got only %s", format_hex_pretty(out, BURST).c_str(),
               format_hex_pretty(in, got).c_str());
    }
    this->test_echo_ = got == BURST ? 1 : 0;

    // Drop anything late or extra, so it does not shift the next burst
    vTaskDelay(pdMS_TO_TICKS(20));
    drain();

    if (millis() - reported >= 10000) {
      ESP_LOGI(TAG, "Adapter test: %" PRIu32 " of %" PRIu32 " bursts echoed (%" PRIu32 " exactly), %" PRIu32 " silent",
               echoed, bursts, exact, silent);
      reported = millis();
    }
  }

  char text[128];
  if (echoed == bursts && exact == bursts && noise == 0) {
    snprintf(text, sizeof(text), "Echo clean in all %" PRIu32 " bursts", bursts);
  } else if (echoed == bursts && noise > 0) {
    snprintf(text, sizeof(text), "Echo in all %" PRIu32 " bursts, but %u bytes on a quiet line: stray light?", bursts,
             static_cast<unsigned>(noise));
  } else if (echoed == bursts) {
    snprintf(text, sizeof(text),
             "Echo in all %" PRIu32 " bursts, garbled in %" PRIu32 ": a weak reflection, enough to poll", bursts,
             bursts - exact);
  } else if (silent == bursts && noise > 0) {
    snprintf(text, sizeof(text), "No echo, and %u bytes on a quiet line: flooded by light, or a short",
             static_cast<unsigned>(noise));
  } else if (silent == bursts) {
    snprintf(text, sizeof(text), "No echo at all: nothing in front to reflect it, or the emitter, receiver or a wire");
  } else if (echoed == 0) {
    // Bytes came, but never all of them: the receiver works, and sees too little of the emitter or
    // something else, such as a torch held to the lens to check it
    snprintf(text, sizeof(text),
             "No full echo, but the receiver saw light in %" PRIu32 " of %" PRIu32 " bursts: too little reflected",
             bursts - silent, bursts);
  } else {
    snprintf(text, sizeof(text),
             "Echo missing in %" PRIu32 " of %" PRIu32 " bursts: a loose contact, stray light or too little reflected",
             bursts - echoed, bursts);
  }
  ESP_LOGI(TAG, "Adapter test: %s", text);

  xSemaphoreTake(this->lock_, portMAX_DELAY);
  this->test_result_ = text;
  this->has_test_result_ = true;
  xSemaphoreGive(this->lock_);

  this->test_echo_ = -1;
  this->testing_ = false;
}

void FreeMDUComponent::publish_adapter_test_(const char *text) {
#ifdef USE_TEXT_SENSOR
  if (this->adapter_test_sensor_ != nullptr)
    this->adapter_test_sensor_->publish_state(text);
#endif
}

void FreeMDUComponent::loop() {
  const int8_t echo = this->test_echo_;
  if (echo != this->published_echo_) {
    this->published_echo_ = echo;
#ifdef USE_BINARY_SENSOR
    if (this->echo_sensor_ != nullptr)
      this->echo_sensor_->publish_state(echo == 1);
#endif
  }

  if (this->has_test_result_) {
    xSemaphoreTake(this->lock_, portMAX_DELAY);
    const std::string text = std::move(this->test_result_);
    this->has_test_result_ = false;
    xSemaphoreGive(this->lock_);
    this->publish_adapter_test_(text.c_str());
  }

  // While the appliance does not answer, try again sooner than update_interval: it shows up within
  // a second of being switched on, and while the sensor is being aligned, the result is immediate
  const uint32_t retry = this->reconnect_interval_ms_;
  if (!this->connected_ && retry > 0 && retry < this->get_update_interval() && !this->busy_ &&
      millis() - this->last_attempt_ >= retry)
    this->update();

  if (!this->has_pending_)
    return;

  PollResult result;

  xSemaphoreTake(this->lock_, portMAX_DELAY);
  std::swap(result, this->pending_);
  this->has_pending_ = false;
  xSemaphoreGive(this->lock_);

  this->publish_(result);
}

void FreeMDUComponent::publish_(const PollResult &result) {
  const bool connected = result.status == FMDU_OK;

  if (!result.memory.empty())
    this->log_memory_(result.memory);

  if (this->first_publish_ || result.status != this->last_status_) {
    if (connected) {
      ESP_LOGI(TAG, "Connected to %s, software ID %u", result.device_kind.c_str(), result.software_id);
    } else if (result.status == FMDU_ERR_UNKNOWN_DEVICE) {
      ESP_LOGW(TAG, "Software ID %u is not supported by FreeMDU", result.software_id);
    } else {
      ESP_LOGW(TAG, "Poll failed: %s", fmdu_status_str(result.status));
    }
  }

#ifdef USE_TEXT_SENSOR
  if (this->status_sensor_ != nullptr) {
    const char *status = fmdu_status_str(result.status);
    if (!this->status_sensor_->has_state() || this->status_sensor_->state != status)
      this->status_sensor_->publish_state(status);
  }
  if (this->software_id_sensor_ != nullptr && result.software_id != 0) {
    const std::string id = std::to_string(result.software_id);
    if (!this->software_id_sensor_->has_state() || this->software_id_sensor_->state != id)
      this->software_id_sensor_->publish_state(id);
  }
  if (this->device_kind_sensor_ != nullptr && connected &&
      (!this->device_kind_sensor_->has_state() || this->device_kind_sensor_->state != result.device_kind))
    this->device_kind_sensor_->publish_state(result.device_kind);
#endif
#ifdef USE_BINARY_SENSOR
  if (this->connected_sensor_ != nullptr)
    this->connected_sensor_->publish_state(connected);
#endif

  if (!connected) {
    const uint32_t now = millis();

    // Log every value again once the appliance answers
    std::fill(this->logged_.begin(), this->logged_.end(), std::string());

    if (this->connected_) {
      // First failure after a good poll: keep the values for now
      this->failing_since_ = now;
      this->holding_ = true;
    }
    if (this->first_publish_ || (this->holding_ && now - this->failing_since_ >= this->hold_time_ms_)) {
      this->publish_unknown_();
      this->holding_ = false;
    }
  } else {
    this->holding_ = false;
    this->log_changes_(result);
    for (size_t i = 0; i < this->bindings_.size(); i++) {
      const PropertyBinding &binding = this->bindings_[i];
      const fmdu_value_t &value = result.values[i];
      const bool ok = result.ok[i];

#ifdef USE_SENSOR
      if (binding.sensor != nullptr) {
        float state = NAN;
        if (ok && value.kind != FMDU_VALUE_STRING)
          state = static_cast<float>(value.kind == FMDU_VALUE_SENSOR && binding.target ? value.target : value.number);
        const float current = binding.sensor->state;
        if (!binding.sensor->has_state() || !(current == state || (std::isnan(current) && std::isnan(state))))
          binding.sensor->publish_state(state);
      }
#endif
#ifdef USE_TEXT_SENSOR
      if (binding.text_sensor != nullptr) {
        if (ok) {
          // Only names are reworded, preferring the implementation's own display name;
          // numbers, dates and durations keep their own text
          std::string text = value.text;
          if (binding.format == TextFormat::TEXT && value.kind == FMDU_VALUE_STRING)
            text = value.name[0] != '\0' ? std::string(value.name) : humanize(value.text);
          if (!binding.text_sensor->has_state() || binding.text_sensor->state != text)
            binding.text_sensor->publish_state(text);
        } else if (binding.text_sensor->has_state()) {
          binding.text_sensor->set_has_state(false);
#ifdef USE_CONTROLLER_REGISTRY
          ControllerRegistry::notify_text_sensor_update(binding.text_sensor);
#endif
        }
      }
#endif
#ifdef USE_BINARY_SENSOR
      if (binding.binary_sensor != nullptr) {
        if (ok) {
          binding.binary_sensor->publish_state(value.number != 0);
        } else {
          binding.binary_sensor->invalidate_state();
        }
      }
#endif
    }
  }

#ifdef USE_SENSOR
  this->update_energy_(result, connected);
#endif

  this->connected_ = connected;
  this->last_status_ = result.status;
  this->first_publish_ = false;
}

void FreeMDUComponent::log_changes_(const PollResult &result) {
  if (this->logged_.size() != this->bindings_.size())
    this->logged_.resize(this->bindings_.size());

  for (size_t i = 0; i < this->bindings_.size(); i++) {
    if (!result.ok[i])
      continue;
    // Several entities may show one property; log it once
    const std::string &property = this->bindings_[i].property;
    bool seen = false;
    for (size_t j = 0; j < i && !seen; j++)
      seen = this->bindings_[j].property == property;
    if (seen)
      continue;

    const fmdu_value_t &value = result.values[i];
    std::string text = value.text;
    if (value.name[0] != '\0' && strcmp(value.name, value.text) != 0)
      text = text + " (" + value.name + ")";
    if (text != this->logged_[i]) {
      ESP_LOGD(TAG, "%s: %s", property.c_str(), text.empty() ? "(none)" : text.c_str());
      this->logged_[i] = std::move(text);
    }
  }
}

size_t FreeMDUComponent::require_property_(const char *property) {
  for (size_t i = 0; i < this->bindings_.size(); i++) {
    if (this->bindings_[i].property == property)
      return i;
  }

  PropertyBinding binding;
  binding.property = property;
  this->bindings_.push_back(std::move(binding));
  return this->bindings_.size() - 1;
}

#ifdef USE_SENSOR
// Property IDs and values the estimate relies on. They are shared by the device implementations:
// every one has an active_actuators property with a Heater flag, and the washing machines report
// their operating state with these names.
static const char *const PROPERTY_ACTIVE_ACTUATORS = "active_actuators";
static const char *const PROPERTY_OPERATING_STATE = "operating_state";
static const char *const PROPERTY_TACHOMETER_SPEED = "tachometer_speed";
// Offered by implementations that know their appliance's power draw, such as software ID 457
static const char *const PROPERTY_ESTIMATED_POWER = "estimated_power";
static const char *const ACTUATOR_HEATER = "Heater";
static const char *const STATE_RUNNING = "ProgramRunning";
static const char *const STATE_FINISHED = "ProgramFinished";

/// Whether a flag set formatted as "A | B | C" contains `flag`.
static bool has_flag(const char *flags, const char *flag) {
  const size_t len = strlen(flag);

  for (const char *p = flags; (p = strstr(p, flag)) != nullptr; p += len) {
    const bool starts = p == flags || p[-1] == ' ';
    const bool ends = p[len] == '\0' || p[len] == ' ';
    if (starts && ends)
      return true;
  }
  return false;
}

void FreeMDUComponent::set_energy_model(float heater_power, float running_power, float standby_power,
                                        float drum_power_per_1000_rpm) {
  this->energy_enabled_ = true;
  this->heater_power_ = heater_power;
  this->running_power_ = running_power;
  this->standby_power_ = standby_power;
  this->drum_power_per_1000_rpm_ = drum_power_per_1000_rpm;
}

void FreeMDUComponent::setup_energy_() {
  if (!this->energy_enabled_)
    return;

  // The operating state marks the program for program_energy in either case
  this->state_index_ = static_cast<int>(this->require_property_(PROPERTY_OPERATING_STATE));
  if (this->heater_power_ <= 0.0f) {
    this->power_index_ = static_cast<int>(this->require_property_(PROPERTY_ESTIMATED_POWER));
  } else {
    this->actuators_index_ = static_cast<int>(this->require_property_(PROPERTY_ACTIVE_ACTUATORS));
    if (this->drum_power_per_1000_rpm_ > 0.0f)
      this->drum_index_ = static_cast<int>(this->require_property_(PROPERTY_TACHOMETER_SPEED));
  }

  sensor::Sensor *owner = this->energy_sensor_ != nullptr ? this->energy_sensor_ : this->program_energy_sensor_;
  if (owner != nullptr) {
    this->energy_pref_ = owner->make_entity_preference<EnergyState>(1);
    EnergyState state;
    EnergyStateV0 old;
    ESPPreferenceObject old_pref = owner->make_entity_preference<EnergyStateV0>();
    if (this->energy_pref_.load(&state) && std::isfinite(state.total_kwh) && state.total_kwh >= 0.0f) {
      this->energy_ = state;
    } else if (old_pref.load(&old) && std::isfinite(old.total_kwh) && old.total_kwh >= 0.0f) {
      this->energy_ = {old.total_kwh, old.program_start_kwh, old.in_program, NAN};
      this->energy_pref_.save(&this->energy_);
    }
    this->saved_total_kwh_ = this->energy_.total_kwh;
  }

  if (this->energy_sensor_ != nullptr)
    this->energy_sensor_->publish_state(this->energy_.total_kwh);
  if (this->program_energy_sensor_ != nullptr && std::isfinite(this->energy_.last_program_kwh))
    this->program_energy_sensor_->publish_state(this->energy_.last_program_kwh);
  if (this->power_sensor_ != nullptr)
    this->power_sensor_->publish_state(0.0f);
}

void FreeMDUComponent::update_energy_(const PollResult &result, bool connected) {
  if (!this->energy_enabled_)
    return;

  // Integrate the power of the interval that just ended (left rectangle rule)
  const uint32_t now = millis();
  if (this->power_since_ != 0)
    this->energy_.total_kwh += this->power_ * static_cast<float>(now - this->power_since_) / 3.6e9f;
  this->power_since_ = now;

  bool persist = false;

  if (connected) {
    const int act = this->actuators_index_;
    const size_t state = this->state_index_;
    const bool has_state = result.ok[state];
    if (!has_state && this->program_energy_sensor_ != nullptr && !this->program_energy_warned_) {
      ESP_LOGW(TAG, "Software ID %u reports no operating state: program energy cannot be measured", result.software_id);
      this->program_energy_warned_ = true;
    }
    if (this->power_index_ >= 0) {
      // The device implementation's own estimate
      const bool ok = result.ok[this->power_index_];
      if (!ok && !this->device_power_warned_) {
        ESP_LOGW(TAG, "Software ID %u reports no estimated power: set heater_power for the energy estimate",
                 result.software_id);
        this->device_power_warned_ = true;
      }
      this->power_ = ok ? static_cast<float>(result.values[this->power_index_].number) : 0.0f;
    } else {
      const bool heating = result.ok[act] && has_flag(result.values[act].text, ACTUATOR_HEATER);
      // Without an operating state (software IDs 517 and 605), any active actuator counts as running
      const bool running = has_state ? strcmp(result.values[state].text, STATE_RUNNING) == 0
                                     : result.ok[act] && result.values[act].text[0] != '\0';

      this->power_ = (running ? this->running_power_ : this->standby_power_) + (heating ? this->heater_power_ : 0.0f);

      // The motor draws more the faster the drum turns, most of all during the final spin
      if (this->drum_index_ >= 0 && result.ok[this->drum_index_] &&
          result.values[this->drum_index_].kind == FMDU_VALUE_SENSOR)
        this->power_ +=
            this->drum_power_per_1000_rpm_ * static_cast<float>(result.values[this->drum_index_].number) / 1000.0f;
    }

    if (has_state) {
      const char *text = result.values[state].text;
      if (strcmp(text, STATE_RUNNING) == 0) {
        if (!this->energy_.in_program) {
          this->energy_.program_start_kwh = this->energy_.total_kwh;
          this->energy_.in_program = true;
          persist = true;
        }
      } else if (this->energy_.in_program) {
        if (strcmp(text, STATE_FINISHED) == 0) {
          this->energy_.last_program_kwh = this->energy_.total_kwh - this->energy_.program_start_kwh;
          if (this->program_energy_sensor_ != nullptr)
            this->program_energy_sensor_->publish_state(this->energy_.last_program_kwh);
        }
        // Finished or aborted, e.g. the door opened or the dial turned to the end
        this->energy_.in_program = false;
        persist = true;
      }
    }
  } else if (!this->holding_) {
    // Switched off, or no answer for longer than hold_time; while holding, the last power stands
    this->power_ = 0.0f;
  }

  if (this->power_sensor_ != nullptr &&
      (!this->power_sensor_->has_state() || this->power_sensor_->state != this->power_))
    this->power_sensor_->publish_state(this->power_);
  // Publish the total in steps of 0.1 Wh rather than on every poll
  if (this->energy_sensor_ != nullptr &&
      (!this->energy_sensor_->has_state() || this->energy_.total_kwh - this->energy_sensor_->state >= 0.0001f))
    this->energy_sensor_->publish_state(this->energy_.total_kwh);

  // Saved to flash in batches by the preferences backend; skip changes below 1 Wh
  if (persist || this->energy_.total_kwh - this->saved_total_kwh_ >= 0.001f) {
    this->energy_pref_.save(&this->energy_);
    this->saved_total_kwh_ = this->energy_.total_kwh;
  }
}
#endif

void FreeMDUComponent::publish_unknown_() {
  for (const PropertyBinding &binding : this->bindings_) {
#ifdef USE_SENSOR
    if (binding.sensor != nullptr && !std::isnan(binding.sensor->state))
      binding.sensor->publish_state(NAN);
#endif
#ifdef USE_TEXT_SENSOR
    if (binding.text_sensor != nullptr && binding.text_sensor->has_state()) {
      // Reported to Home Assistant as unknown, the same as a NaN sensor
      binding.text_sensor->set_has_state(false);
#ifdef USE_CONTROLLER_REGISTRY
      ControllerRegistry::notify_text_sensor_update(binding.text_sensor);
#endif
    }
#endif
#ifdef USE_BINARY_SENSOR
    if (binding.binary_sensor != nullptr)
      binding.binary_sensor->invalidate_state();
#endif
  }
}

void FreeMDUComponent::dump_config() {
  ESP_LOGCONFIG(TAG,
                "FreeMDU:\n"
                "  Discard echo: %s\n"
                "  Timeout: %" PRIu32 " ms\n"
                "  Protocol log: %s\n"
                "  Hold time: %" PRIu32 " ms\n"
                "  Reconnect interval: %" PRIu32 " ms\n"
                "  Properties: %u",
                YESNO(this->discard_echo_), this->timeout_ms_, YESNO(this->protocol_log_), this->hold_time_ms_,
                this->reconnect_interval_ms_,
                static_cast<unsigned>(this->bindings_.size()));
  for (const PropertyBinding &binding : this->bindings_)
    ESP_LOGCONFIG(TAG, "    %s", binding.property.c_str());
  if (this->memory_log_length_ > 0)
    ESP_LOGCONFIG(TAG, "  Memory log: %" PRIu32 " bytes from 0x%04" PRIx32, this->memory_log_length_,
                  this->memory_log_address_);
#ifdef USE_SENSOR
  if (this->energy_enabled_ && this->heater_power_ <= 0.0f) {
    ESP_LOGCONFIG(TAG,
                  "  Energy estimate: the device's own power\n"
                  "    Total: %.3f kWh",
                  this->energy_.total_kwh);
  } else if (this->energy_enabled_) {
    ESP_LOGCONFIG(TAG,
                  "  Energy estimate:\n"
                  "    Heater power: %.0f W\n"
                  "    Running power: %.0f W\n"
                  "    Standby power: %.1f W\n"
                  "    Drum power: %.0f W per 1000 rpm\n"
                  "    Total: %.3f kWh",
                  this->heater_power_, this->running_power_, this->standby_power_, this->drum_power_per_1000_rpm_,
                  this->energy_.total_kwh);
  }
  if (this->energy_enabled_) {
    LOG_SENSOR("    ", "Power", this->power_sensor_);
    LOG_SENSOR("    ", "Energy", this->energy_sensor_);
    LOG_SENSOR("    ", "Program energy", this->program_energy_sensor_);
  }
#endif
  LOG_UPDATE_INTERVAL(this);
  this->check_uart_settings(2400, 1, uart::UART_CONFIG_PARITY_EVEN, 8);
}

#ifdef USE_SENSOR
void FreeMDUComponent::add_sensor(const char *property, sensor::Sensor *sensor, bool target) {
  PropertyBinding binding;
  binding.property = property;
  binding.sensor = sensor;
  binding.target = target;
  this->bindings_.push_back(std::move(binding));
}
#endif

#ifdef USE_TEXT_SENSOR
void FreeMDUComponent::add_text_sensor(const char *property, text_sensor::TextSensor *text_sensor,
                                       TextFormat format) {
  PropertyBinding binding;
  binding.property = property;
  binding.text_sensor = text_sensor;
  binding.format = format;
  this->bindings_.push_back(std::move(binding));
}

void FreeMDUComponent::set_info_text_sensor(TextInfo info, text_sensor::TextSensor *text_sensor) {
  switch (info) {
    case TextInfo::SOFTWARE_ID:
      this->software_id_sensor_ = text_sensor;
      break;
    case TextInfo::DEVICE_KIND:
      this->device_kind_sensor_ = text_sensor;
      break;
    case TextInfo::STATUS:
      this->status_sensor_ = text_sensor;
      break;
    case TextInfo::ADAPTER_TEST:
      this->adapter_test_sensor_ = text_sensor;
      break;
  }
}
#endif

#ifdef USE_BINARY_SENSOR
void FreeMDUComponent::add_binary_sensor(const char *property, binary_sensor::BinarySensor *binary_sensor) {
  PropertyBinding binding;
  binding.property = property;
  binding.binary_sensor = binary_sensor;
  this->bindings_.push_back(std::move(binding));
}
#endif

}  // namespace esphome::freemdu
