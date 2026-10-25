// FreeMDU front panel card for Home Assistant.
//
// Draws a washing machine's front panel from the entities of the FreeMDU ESPHome packages:
// the seven-segment display, its indicator lamps, the program, the progress through the
// phases, the option buttons, the spin setting and the status lamps. It shows whatever the
// machine provides and leaves out the rest, so it works with any software ID. A preset per
// software ID adds the panel's own buttons and lamps, so that unlit ones are drawn too.
//
// One file, no build step and no dependencies. See esphome/README.md, "Front panel card".

const VERSION = "0.4.0";

// Entities the card looks for, by the object ID the packages give them
const SENSORS = [
  "software_id",
  "status",
  "operating_state",
  "program",
  "program_type",
  "options",
  "spin_setting",
  "phase",
  "display_contents",
  "display_indicator",
  "remaining_time",
  "start_delay",
  "temperature",
  "program_temperature",
  "target_temperature",
  "program_spin_speed",
  "drum_speed",
  "drum_speed_target",
  "water_level",
  "water_level_target",
  "load_level",
  "program_max_load",
  "estimated_power",
  "last_program_energy",
  "active_actuators",
  "adapter_status",
];
const BINARY_SENSORS = ["connected", "door_locked", "door_interlock", "spinning"];
// Fault entities are found by pattern rather than by name, since each driver has its own set
const FAULT_PATTERN = /(^|_)(fault_f\d+.*|detergent_overdose)$/;

// What a preset may hold, all optional:
//   digits:     number of digits on the display
//   groups:     how the digits are grouped, left to right, such as [1, 2] for hours and minutes;
//               the display text then fills them from the right, with a gap between groups
//   indicators: lamps beside the display, lit when Display Indicator matches one of `match`
//   options:    the option buttons in panel order, lit when Options holds one of `match`
//   lamps:      other lamps lit from Options, such as a door button
//   delay_first: the Start delay lamp sits above the others beside the display
//   spin:       the spin selector's lamps: a number is a speed in rpm, a string a named setting
//
// Values are compared ignoring case, spaces and punctuation, so both the `code` and the `text`
// format of the text sensors match.
const PRESETS = {
  // Miele W 471 and other machines with software ID 457
  457: {
    digits: 3,
    indicators: [
      { label: "Load %", match: ["Load"] },
      { label: "Detergent %", match: ["Detergent"] },
      { label: "Remaining time", match: ["RemainingTime"] },
    ],
    options: [
      { label: "Soak", match: ["Soak"] },
      { label: "Pre-wash", match: ["PreWash"] },
      { label: "Intensive", match: ["IntensiveShort"] },
      { label: "Water plus", match: ["WaterPlus"] },
    ],
    spin: [1600, 1400, 1200, 900, 600, 400, "Rinse hold", "No spin"],
  },
  // Miele W 925 and other machines with software ID 132. The display has one digit for the
  // hours and two for the minutes, with Start delay above Remaining time beside it. The door
  // button sits in the same row as the options, left of the on/off button, which reports nothing
  132: {
    digits: 3,
    groups: [1, 2],
    delay_first: true,
    options: [
      { label: "Door", match: ["DoorButton"] },
      { label: "Soak", match: ["Soak"] },
      { label: "Pre-wash", match: ["PreWash"] },
      { label: "Water plus", match: ["WaterPlus"] },
      { label: "Intensive/short", match: ["IntensiveShort"] },
    ],
    spin: [1450, 1200, 900, 600, 400, "Rinse hold", "No final spin"],
  },
};

// Progress through a program, matched against the phase names every driver uses
const STAGES = [
  { label: "Pre-wash", match: /soak|pre-?wash/i },
  { label: "Main wash", match: /main/i },
  { label: "Rinse", match: /rinse/i },
  { label: "Spin", match: /drain|spin/i },
  { label: "Finish", match: /anti-?crease|finish/i },
];

// Seven-segment patterns, segments a to g, for every character the drivers decode
const GLYPHS = {
  0: "abcdef", 1: "bc", 2: "abdeg", 3: "abcdg", 4: "bcfg", 5: "acdfg", 6: "acdefg", 7: "abc",
  8: "abcdefg", 9: "abcdfg", A: "abcefg", b: "cdefg", C: "adef", c: "deg", d: "bcdeg",
  E: "adefg", F: "aefg", H: "bcefg", h: "cefg", J: "bcde", L: "def", n: "ceg", o: "cdeg",
  P: "abefg", r: "eg", U: "bcdef", u: "cde", y: "bcdfg", "-": "g", "=": "dg", "°": "abfg",
  " ": "",
};

const T = 3.5; // half the segment thickness
const hseg = (x1, x2, y) =>
  `${x1},${y} ${x1 + T},${y - T} ${x2 - T},${y - T} ${x2},${y} ${x2 - T},${y + T} ${x1 + T},${y + T}`;
const vseg = (x, y1, y2) =>
  `${x},${y1} ${x + T},${y1 + T} ${x + T},${y2 - T} ${x},${y2} ${x - T},${y2 - T} ${x - T},${y1 + T}`;
const SEGMENTS = {
  a: hseg(7, 33, 6),
  b: vseg(34, 7, 34),
  c: vseg(34, 36, 63),
  d: hseg(7, 33, 64),
  e: vseg(6, 36, 63),
  f: vseg(6, 7, 34),
  g: hseg(7, 33, 35),
};
const DIGIT_WIDTH = 46;
const GAP_WIDTH = 14;

const norm = (s) => String(s).toLowerCase().replace(/[^a-z0-9]/g, "");
const esc = (s) =>
  String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
const NO_VALUE = new Set(["unknown", "unavailable", "none", ""]);

// Splits the display text into digit cells: a point lights the previous digit's point, and a
// space is a gap, as between the hours and the minutes on some panels. With `groups`, the digits
// fill the groups from the right and the gaps sit between the groups, wherever the text has them
function displayCells(text, digits, groups) {
  const cells = [];
  for (const ch of text ?? "") {
    if (ch === ".") {
      const last = cells.findLast((c) => !c.gap);
      if (last && !last.dp) last.dp = true;
      else cells.push({ ch: " ", dp: true });
    } else if (ch === " ") {
      cells.push({ gap: true });
    } else {
      cells.push({ ch: ch in GLYPHS ? ch : "-", dp: false });
    }
  }
  const count = cells.filter((c) => !c.gap).length;
  const total = groups ? groups.reduce((a, b) => a + b, 0) : 0;
  if (groups && count <= total) {
    const lit = cells.filter((c) => !c.gap);
    while (lit.length < total) lit.unshift({ ch: " ", dp: false });
    const out = [];
    let i = 0;
    groups.forEach((n, g) => {
      if (g) out.push({ gap: true });
      out.push(...lit.slice(i, i + n));
      i += n;
    });
    return out;
  }
  for (let i = count; i < digits; i++) cells.unshift({ ch: " ", dp: false });
  return cells;
}

function renderDisplay(text, digits, groups) {
  const cells = displayCells(text, digits, groups);
  let x = 0;
  const parts = [];
  for (const cell of cells) {
    if (cell.gap) {
      x += GAP_WIDTH;
      continue;
    }
    const lit = GLYPHS[cell.ch] ?? "";
    const segs = Object.entries(SEGMENTS)
      .map(([name, points]) => `<polygon class="${lit.includes(name) ? "on" : "off"}" points="${points}"/>`)
      .join("");
    const dp = `<circle class="${cell.dp ? "on" : "off"}" cx="41" cy="64" r="3.2"/>`;
    parts.push(`<g transform="translate(${x + 4} 0) skewX(-6)">${segs}${dp}</g>`);
    x += DIGIT_WIDTH;
  }
  return `<svg class="digits" viewBox="0 0 ${x + 4} 70" role="img" aria-label="${esc(text ?? "")}">${parts.join("")}</svg>`;
}

function formatClock(hass, date) {
  return date.toLocaleTimeString(hass?.locale?.language || undefined, { hour: "2-digit", minute: "2-digit" });
}

class FreemduPanelCard extends HTMLElement {
  static getStubConfig(hass) {
    // The first device with a FreeMDU software ID sensor
    const entity = Object.values(hass?.entities ?? {}).find((e) => e.entity_id.endsWith("_software_id") && e.device_id);
    return entity ? { device: entity.device_id } : { prefix: "washing_machine" };
  }

  setConfig(config) {
    if (!config || (!config.device && !config.prefix && !config.entities)) {
      throw new Error("Set `device` (a device ID) or `prefix` (such as washing_machine)");
    }
    this._config = config;
    this._entities = null;
    this._lastKey = null;
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
      this.shadowRoot.addEventListener("click", (ev) => this._onClick(ev));
    }
    if (this._hass) this._render();
  }

  set hass(hass) {
    const devicesChanged = this._hass?.entities !== hass.entities;
    this._hass = hass;
    if (devicesChanged) this._entities = null;
    this._render();
  }

  getCardSize() {
    return 7;
  }

  getGridOptions() {
    return { columns: 12, min_columns: 6 };
  }

  // Maps each key to an entity ID: from the device's entities, from the prefix, and from
  // explicit `entities:` overrides, in rising order of precedence
  _resolve() {
    const hass = this._hass;
    const cfg = this._config;
    const found = {};
    const keys = [...SENSORS.map((k) => ["sensor", k]), ...BINARY_SENSORS.map((k) => ["binary_sensor", k])];
    const faults = [];

    if (cfg.device) {
      const own = Object.values(hass.entities ?? {}).filter((e) => e.device_id === cfg.device);
      const all = keys.map(([, k]) => k);
      for (const [domain, key] of keys) {
        // Ends with the key, and with no longer key that ends the same way: so that
        // "temperature" does not take "program_temperature"
        const match = own.find((e) => {
          const [d, object] = e.entity_id.split(".");
          if (d !== domain || !(object === key || object.endsWith(`_${key}`))) return false;
          return !all.some((other) => other !== key && other.endsWith(`_${key}`) && object.endsWith(other));
        });
        if (match) found[key] = match.entity_id;
      }
      for (const e of own) {
        const [d, object] = e.entity_id.split(".");
        if (d === "binary_sensor" && FAULT_PATTERN.test(object)) faults.push(e.entity_id);
      }
    } else if (cfg.prefix) {
      for (const [domain, key] of keys) {
        const id = `${domain}.${cfg.prefix}_${key}`;
        if (hass.states[id]) found[key] = id;
      }
      for (const id of Object.keys(hass.states)) {
        const [d, object] = id.split(".");
        if (d === "binary_sensor" && object.startsWith(`${cfg.prefix}_`) && FAULT_PATTERN.test(object)) faults.push(id);
      }
    }

    Object.assign(found, cfg.entities ?? {});
    for (const id of cfg.faults ?? []) if (!faults.includes(id)) faults.push(id);
    return { map: found, faults };
  }

  _state(key) {
    const id = this._entities.map[key];
    return id ? this._hass.states[id] : undefined;
  }

  _val(key) {
    const s = this._state(key)?.state;
    return s === undefined || NO_VALUE.has(s) ? null : s;
  }

  _num(key) {
    const v = this._val(key);
    const n = v === null ? NaN : Number(v);
    return Number.isFinite(n) ? n : null;
  }

  _has(key) {
    return this._state(key) !== undefined;
  }

  _flags(key) {
    const v = this._val(key);
    return new Set(v ? v.split(/\s*[,|]\s*/).map(norm).filter(Boolean) : []);
  }

  _deviceName() {
    if (this._config.name) return this._config.name;
    const device = this._hass.devices?.[this._config.device];
    return device?.name_by_user || device?.name || "Washing machine";
  }

  _preset() {
    const forced = this._config.preset;
    if (forced === "generic") return {};
    const id = forced ?? this._num("software_id");
    return PRESETS[id] ?? {};
  }

  _render() {
    if (!this._config || !this._hass || !this.shadowRoot) return;
    if (!this._entities) this._entities = this._resolve();

    // Re-render only when one of the card's entities changed
    const ids = [...Object.values(this._entities.map), ...this._entities.faults];
    const key = ids.map((id) => this._hass.states[id]?.last_updated ?? "").join("|") + `|${new Date().getMinutes()}`;
    if (key === this._lastKey) return;
    this._lastKey = key;

    const night = this._hass?.themes?.darkMode ? "night" : "";
    this.shadowRoot.innerHTML = `<style>${STYLE}</style><ha-card class="${night}">${this._body()}</ha-card>`;
  }

  _online() {
    if (this._has("connected")) return this._state("connected").state === "on";
    return ["program", "phase", "display_contents", "status"].some((k) => this._val(k) !== null);
  }

  _status(online) {
    if (!online) return { text: "Off", tone: "off" };
    const status = this._val("status") ?? this._val("operating_state");
    const phase = this._val("phase");
    if (phase && /delayed/i.test(phase)) return { text: "Delayed start", tone: "wait" };
    if (status) {
      const n = norm(status);
      const tone = n.includes("running") ? "run" : n.includes("finish") ? "done" : "idle";
      return { text: this._human(status), tone };
    }
    // No status sensor: tell from the phase
    if (!phase || /not started/i.test(phase)) return { text: "Ready", tone: "idle" };
    if (/anti-?crease|finish/i.test(phase)) return { text: "Finished", tone: "done" };
    return { text: "Running", tone: "run" };
  }

  // The `code` format comes in CamelCase
  _human(s) {
    return /\s/.test(s) || !/[a-z][A-Z]/.test(s) ? s : s.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/ (\w)/g, (m) => m.toLowerCase());
  }

  _body() {
    const preset = this._preset();
    const online = this._online();
    const status = this._status(online);
    const phase = online ? this._val("phase") : null;
    const program = online ? this._val("program") : null;
    const swid = this._val("software_id");

    return `
      <div class="head">
        <div class="name">${esc(this._deviceName())}</div>
        <div class="pill ${status.tone}" data-entity="${esc(this._entities.map.status ?? this._entities.map.phase ?? "")}"><i></i>${esc(status.text)}</div>
      </div>
      <div class="program" data-entity="${esc(this._entities.map.program ?? "")}">
        <div class="title">${esc(program ? this._human(program) : online ? "No program" : "—")}</div>
        <div class="sub">${esc(phase ? this._human(phase) : "")}</div>
      </div>
      <div class="panel ${online ? "" : "off"}">
        ${this._displayBlock(preset, online)}
        ${this._indicators(preset, online)}
      </div>
      ${online ? `
        ${this._times()}
        ${this._stages(phase)}
        ${this._options(preset)}
        ${this._spin(preset)}
        ${this._statusLamps(preset)}
        ${this._readouts()}` : ""}
      <div class="foot">${swid ? `Software ID ${esc(swid)}` : ""}</div>`;
  }

  _displayBlock(preset, online) {
    const text = online ? this._val("display_contents") : null;
    const digits = preset.digits ?? Math.max(3, (text ?? "").replace(/[. ]/g, "").length);
    return `<div class="display ${online ? "" : "dark"}" data-entity="${esc(this._entities.map.display_contents ?? "")}">${renderDisplay(text ?? "", digits, preset.groups)}</div>`;
  }

  _lamp(label, lit, extra = "") {
    return `<span class="lamp ${lit ? "lit" : ""} ${extra}"><i></i>${esc(label)}</span>`;
  }

  // A button on the panel, with the lamp it lights
  _button(label, lit) {
    return `<span class="button ${lit ? "lit" : ""}"><i></i>${esc(label)}</span>`;
  }

  _indicators(preset, online) {
    const lamps = [];
    if (preset.indicators && this._has("display_indicator")) {
      const shown = norm(this._val("display_indicator") ?? "");
      for (const ind of preset.indicators) {
        lamps.push(this._lamp(ind.label, online && ind.match.some((m) => norm(m) === shown || norm(ind.label) === shown)));
      }
    } else if (this._has("display_indicator")) {
      const shown = this._val("display_indicator");
      if (shown && norm(shown) !== "off") lamps.push(this._lamp(this._human(shown), online));
    }
    if (!lamps.length && this._has("remaining_time")) {
      lamps.push(this._lamp("Remaining time", online && this._num("remaining_time") !== null));
    }
    if (this._has("start_delay") || this._has("phase")) {
      const delayed = this._num("start_delay") !== null || /delayed/i.test(this._val("phase") ?? "");
      const lamp = this._lamp("Start delay", online && delayed);
      if (preset.delay_first) lamps.unshift(lamp);
      else lamps.push(lamp);
    }
    return lamps.length ? `<div class="indicators">${lamps.join("")}</div>` : "";
  }

  _times() {
    if (this._status(true).tone === "done") return "";
    const now = new Date();
    const delay = this._num("start_delay");
    const remaining = this._num("remaining_time");
    const parts = [];
    if (delay !== null) {
      parts.push(`<span>starts at <b>${esc(formatClock(this._hass, new Date(now.getTime() + delay * 60000)))}</b></span>`);
    }
    if (remaining !== null) {
      const h = Math.floor(remaining / 60);
      const m = Math.round(remaining % 60);
      const left = h ? `${h} h ${m} min` : `${m} min`;
      parts.push(`<span><b>${left}</b> left</span>`);
      parts.push(`<span>ends at ${esc(formatClock(this._hass, new Date(now.getTime() + remaining * 60000)))}</span>`);
    }
    return parts.length ? `<div class="times">${parts.join("")}</div>` : "";
  }

  _stages(phase) {
    if (!this._has("phase")) return "";
    const current = phase ? STAGES.findIndex((s) => s.match.test(phase)) : -1;
    // Stages before the current one are done, except a pre-wash that was not chosen
    const flags = this._flags("options");
    const prewash = flags.has("prewash") || flags.has("soak");
    const items = STAGES.map((s, i) => {
      const done = current > i && (i > 0 || prewash);
      const cls = i === current ? "lit" : done ? "done" : "";
      return `<span class="stage ${cls}"><i></i>${esc(s.label)}</span>`;
    });
    // The track runs between the first and the last dot, each in the middle of its column
    const span = 100 - 100 / STAGES.length;
    const fill = current > 0 ? (span * current) / (STAGES.length - 1) : 0;
    const style = `--track-inset: ${50 / STAGES.length}%; --track-fill: ${fill}%`;
    return `<div class="stages" style="${style}">${items.join("")}</div>`;
  }

  _options(preset) {
    if (!this._has("options")) return "";
    const flags = this._flags("options");
    const known = new Set([...(preset.options ?? []), ...(preset.lamps ?? [])].flatMap((o) => o.match.map(norm)));
    const lamps = (preset.options ?? []).map((o) => this._button(o.label, o.match.some((m) => flags.has(norm(m)))));
    // Options the preset does not know about show while they are set
    const raw = (this._val("options") ?? "").split(/\s*[,|]\s*/).filter(Boolean);
    for (const name of raw) if (!known.has(norm(name))) lamps.push(this._button(this._human(name), true));
    return lamps.length ? `<div class="group"><span class="label">Options</span><div class="buttons">${lamps.join("")}</div></div>` : "";
  }

  _spin(preset) {
    if (!this._has("spin_setting") && !this._has("program_spin_speed")) return "";
    const setting = this._val("spin_setting");
    if (!preset.spin) {
      return setting ? `<div class="group"><span class="label">Spin</span><div class="buttons">${this._button(this._human(setting), true)}</div></div>` : "";
    }
    // A named setting lights its own lamp; otherwise the speed picks the lamp
    const named = setting && preset.spin.find((s) => typeof s === "string" && norm(s) === norm(setting));
    let rpm = this._num("program_spin_speed");
    // Without a speed sensor, from the setting itself: "1200 rpm" or "Spin1200"
    if (rpm === null && setting) rpm = Number(/(\d{3,4})/.exec(setting)?.[1] ?? NaN);
    const lamps = preset.spin.map((s) => {
      const lit = typeof s === "string" ? s === named : !named && s === rpm;
      return this._button(typeof s === "number" ? String(s) : s, lit);
    });
    return `<div class="group"><span class="label">Spin</span><div class="buttons spin">${lamps.join("")}</div></div>`;
  }

  _statusLamps(preset) {
    const lamps = [];
    const on = (key) => this._state(key)?.state === "on";
    if (this._has("door_locked")) lamps.push(this._lamp("Door locked", on("door_locked")));
    else if (this._has("door_interlock")) lamps.push(this._lamp("Door locked", on("door_interlock")));
    if (this._has("spinning")) lamps.push(this._lamp("Drum turning", on("spinning")));
    if (preset.lamps && this._has("options")) {
      const flags = this._flags("options");
      for (const l of preset.lamps) lamps.push(this._lamp(l.label, l.match.some((m) => flags.has(norm(m)))));
    }
    // Faults show only while they are set
    const prefix = norm(this._deviceName());
    for (const id of this._entities.faults) {
      const s = this._hass.states[id];
      if (s?.state !== "on") continue;
      let label = s.attributes.friendly_name ?? id;
      if (norm(label).startsWith(prefix)) label = label.slice(this._deviceName().length).trim();
      lamps.push(this._lamp(label, true, "fault"));
    }
    return lamps.length ? `<div class="lamps">${lamps.join("")}</div>` : "";
  }

  _readouts() {
    const items = [];
    const unit = (key) => this._state(key)?.attributes?.unit_of_measurement ?? "";
    const pair = (now, target) => {
      const a = this._num(now);
      const b = this._num(target);
      if (a !== null && b !== null) return `${Math.round(a)} / ${Math.round(b)}`;
      if (a !== null) return `${Math.round(a)}`;
      if (b !== null) return `→ ${Math.round(b)}`;
      return null;
    };
    const add = (label, key, value, u) => {
      if (value !== null) items.push(`<div class="readout" data-entity="${esc(this._entities.map[key] ?? "")}"><span>${esc(label)}</span><b>${esc(value)}${u ? ` ${esc(u)}` : ""}</b></div>`);
    };
    const target = this._has("program_temperature") ? "program_temperature" : "target_temperature";
    add("Temperature", this._has("temperature") ? "temperature" : target, pair("temperature", target), unit(target) || unit("temperature") || "°C");
    add("Drum", "drum_speed", pair("drum_speed", "drum_speed_target"), "rpm");
    add("Water", "water_level", pair("water_level", "water_level_target"), unit("water_level"));
    add("Load", "load_level", pair("load_level", "program_max_load"), unit("load_level") || unit("program_max_load"));
    const power = this._num("estimated_power");
    add("Power", "estimated_power", power === null ? null : String(Math.round(power)), unit("estimated_power") || "W");
    const energy = this._num("last_program_energy");
    if (energy !== null && this._status(true).tone === "done") {
      add("Program energy", "last_program_energy", energy.toFixed(2), unit("last_program_energy") || "kWh");
    }
    const working = this._val("active_actuators");
    if (working) items.push(`<div class="readout wide" data-entity="${esc(this._entities.map.active_actuators)}"><span>Working</span><b>${esc(this._human(working))}</b></div>`);
    return items.length ? `<div class="readouts">${items.join("")}</div>` : "";
  }

  _onClick(ev) {
    const el = ev.composedPath().find((n) => n.dataset?.entity !== undefined);
    const entityId = el?.dataset.entity;
    if (!entityId) return;
    this.dispatchEvent(new CustomEvent("hass-more-info", { detail: { entityId }, bubbles: true, composed: true }));
  }
}

const STYLE = `
  /* The look of the machines' front panels: a cream face (W 471, W 925) with a black display
     window, amber digits and lamps, and a green lamp for a finished program, as on the W 471's
     Start button. A dark theme gets a dark grey face. Themes can set --freemdu-led and
     --freemdu-face. */
  :host {
    --_led: var(--freemdu-led, #ffa41b);
    --_green: #8fd61c;
    --_radius: 8px;
  }
  ha-card {
    --_face: var(--freemdu-face, #ebe7dc);
    --_face-low: color-mix(in srgb, var(--_face) 92%, #8a7f68);
    --_ink: #3a3833;
    --_ink-soft: #6f6a60;
    --_rule: rgba(58, 56, 51, 0.16);
    --_key: #f8f6f0;
    --_key-low: #e4e0d5;
    --_key-edge: #c6c0b2;
    --_lamp-off: #6f6b62;
    --_plate: rgba(58, 56, 51, 0.05);
    --ha-card-background: var(--_face);
    padding: 16px; display: flex; flex-direction: column; gap: 14px;
    background: linear-gradient(180deg, var(--_face) 0%, var(--_face-low) 100%);
    color: var(--_ink);
    border: 1px solid var(--_key-edge);
    border-radius: var(--ha-card-border-radius, 12px);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.7), 0 1px 3px rgba(0, 0, 0, 0.12);
  }
  ha-card.night {
    --_face: var(--freemdu-face, #2b2925);
    --_face-low: color-mix(in srgb, var(--_face) 88%, #000);
    --_ink: #ebe6dc;
    --_ink-soft: #a39e93;
    --_rule: rgba(235, 230, 220, 0.13);
    --_key: #3c3934;
    --_key-low: #312f2b;
    --_key-edge: #4c4841;
    --_lamp-off: #57534b;
    --_plate: rgba(235, 230, 220, 0.04);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05), 0 1px 3px rgba(0, 0, 0, 0.4);
  }
  [data-entity]:not([data-entity=""]) { cursor: pointer; }

  .head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .name { font-size: 0.78em; font-weight: 600; color: var(--_ink-soft); text-transform: uppercase; letter-spacing: 0.1em; }
  .pill { position: relative; display: inline-flex; align-items: center; gap: 6px; font-size: 0.8em; font-weight: 500; padding: 3px 10px; border-radius: 999px; color: var(--_ink-soft); white-space: nowrap; overflow: hidden; border: 1px solid var(--_rule); }
  .pill i { width: 7px; height: 7px; border-radius: 50%; background: var(--_lamp-off); }
  .pill.run i { background: var(--_led); box-shadow: 0 0 4px var(--_led), 0 0 9px var(--_led); animation: pulse 1.6s ease-in-out infinite; }
  .pill.wait i { background: var(--_led); box-shadow: 0 0 4px var(--_led); }
  .pill.done i { background: var(--_green); box-shadow: 0 0 4px var(--_green), 0 0 9px var(--_green); }
  .pill.run, .pill.wait, .pill.done { color: var(--_ink); }
  .pill.off i { opacity: 0.5; }

  .program .title { font-size: 1.6em; font-weight: 400; line-height: 1.2; color: var(--_ink); }
  .program .sub { margin-top: 2px; color: var(--_ink-soft); min-height: 1.2em; }

  .panel { display: flex; align-items: center; gap: 18px; flex-wrap: wrap; }
  .display { position: relative; padding: 10px 14px; border-radius: 4px; line-height: 0; overflow: hidden;
    background: linear-gradient(180deg, #1b1712 0%, #0b0907 55%);
    box-shadow: inset 0 2px 5px rgba(0, 0, 0, 0.9), 0 0 0 1px rgba(0, 0, 0, 0.55), 0 1px 0 rgba(255, 255, 255, 0.5); }
  ha-card.night .display { box-shadow: inset 0 2px 5px rgba(0, 0, 0, 0.9), 0 0 0 1px rgba(0, 0, 0, 0.7), 0 1px 0 rgba(255, 255, 255, 0.06); }
  .display::after { content: ""; position: absolute; inset: 0 0 60% 0; background: linear-gradient(180deg, rgba(255, 255, 255, 0.06), rgba(255, 255, 255, 0)); pointer-events: none; }
  .digits { height: 56px; width: auto; }
  .digits .on { fill: var(--_led); filter: drop-shadow(0 0 1.5px var(--_led)) drop-shadow(0 0 6px var(--_led)); }
  .digits .off { fill: var(--_led); opacity: 0.06; }
  .panel.off .display { background: #0b0907; }
  .panel.off .digits .off { opacity: 0.035; }

  .indicators, .lamps { display: flex; gap: 8px 16px; }
  .indicators { flex-direction: column; }
  .lamps { flex-wrap: wrap; }
  .lamp { display: inline-flex; align-items: center; gap: 8px; font-size: 0.86em; color: var(--_ink); white-space: nowrap; }
  .lamp i, .button i { flex: none; width: 7px; height: 7px; border-radius: 50%; background: radial-gradient(circle at 40% 35%, color-mix(in srgb, var(--_lamp-off) 70%, #fff) 0, var(--_lamp-off) 60%); box-shadow: inset 0 1px 1px rgba(0, 0, 0, 0.35); }
  .lamp.lit i, .button.lit i { background: radial-gradient(circle at 38% 35%, #fff6dc 0, var(--_led) 50%); box-shadow: 0 0 4px var(--_led), 0 0 10px var(--_led); }
  .lamp.fault { color: var(--error-color, #db4437); font-weight: 500; }
  .lamp.fault i { animation: pulse 1.2s ease-in-out infinite; }

  .times { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 14px; color: var(--_ink-soft); }
  .times b { font-size: 1.35em; font-weight: 500; color: var(--_ink); font-variant-numeric: tabular-nums; }

  .stages { position: relative; display: flex; padding-top: 2px; }
  .stages::before, .stages::after { content: ""; position: absolute; top: 7px; left: var(--track-inset); height: 2px; border-radius: 1px; }
  .stages::before { right: var(--track-inset); background: var(--_rule); }
  .stages::after { width: var(--track-fill); background: var(--_led); opacity: 0.7; }
  .stage { position: relative; z-index: 1; flex: 1; display: flex; flex-direction: column; align-items: center; gap: 6px; font-size: 0.78em; color: var(--_ink-soft); text-align: center; }
  .stage i { width: 10px; height: 10px; border-radius: 50%; border: 2px solid var(--_key-edge); background: var(--_face); }
  .stage.done i { border-color: var(--_led); background: var(--_led); opacity: 0.7; }
  .stage.lit { color: var(--_ink); font-weight: 500; }
  .stage.lit i { border-color: var(--_led); background: var(--_led); box-shadow: 0 0 0 4px var(--_face), 0 0 10px 4px var(--_led); }

  .group { display: flex; flex-direction: column; gap: 8px; }
  .label { font-size: 0.72em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.1em; color: var(--_ink-soft); }
  .buttons { display: flex; flex-wrap: wrap; gap: 6px; }
  .button { position: relative; display: inline-flex; align-items: center; gap: 7px; padding: 4px 10px 4px 8px; border-radius: 9px; font-size: 0.82em; color: var(--_ink); white-space: nowrap;
    background: linear-gradient(180deg, var(--_key) 0%, var(--_key-low) 100%);
    border: 1px solid var(--_key-edge);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55), 0 1px 1px rgba(0, 0, 0, 0.12); }
  ha-card.night .button { box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06), 0 1px 1px rgba(0, 0, 0, 0.4); }
  .buttons.spin { gap: 6px 14px; }
  .buttons.spin .button { padding: 2px 0; background: none; border: none; box-shadow: none; font-variant-numeric: tabular-nums; }

  .readouts { display: grid; grid-template-columns: repeat(auto-fill, minmax(100px, 1fr)); gap: 8px; }
  .readout { display: flex; flex-direction: column; gap: 3px; padding: 8px 10px; border-radius: var(--_radius); background: var(--_plate); border: 1px solid var(--_rule); }
  .readout.wide { grid-column: 1 / -1; }
  .readout span { font-size: 0.72em; color: var(--_ink-soft); }
  .readout b { font-size: 1.05em; font-weight: 500; color: var(--_ink); font-variant-numeric: tabular-nums; }

  .foot { margin-top: -4px; font-size: 0.72em; color: var(--_ink-soft); text-align: right; opacity: 0.8; }
  .foot:empty { display: none; }

  @keyframes pulse { 50% { opacity: 0.35; } }
  @media (prefers-reduced-motion: reduce) { .pill.run i, .lamp.fault i { animation: none; } }
`;

// Loaded twice when a dashboard names both a copy of this file and the component's loader
if (!customElements.get("freemdu-panel-card")) {
  customElements.define("freemdu-panel-card", FreemduPanelCard);
  window.customCards = window.customCards || [];
  window.customCards.push({
    type: "freemdu-panel-card",
    name: "FreeMDU front panel",
    description: "The front panel of a washing machine connected with FreeMDU",
  });
  console.info(`%c FREEMDU-PANEL-CARD %c ${VERSION} `, "color: #1b1712; background: #ffa41b", "");
}
