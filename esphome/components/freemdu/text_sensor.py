import esphome.codegen as cg
from esphome.components import text_sensor
import esphome.config_validation as cv

from . import (
    CONF_FREEMDU_ID,
    CONF_PROPERTY,
    FREEMDU_CHILD_SCHEMA,
    PROPERTY_ID,
    freemdu_ns,
)

DEPENDENCIES = ["freemdu"]

CONF_INFO = "info"
CONF_FORMAT = "format"
TextFormat = freemdu_ns.enum("TextFormat", is_class=True)
FORMATS = {
    # The device implementation's own names, e.g. ProgramRunning; stable, for automations
    "code": TextFormat.CODE,
    # A name for people: the device implementation's display name where it has one
    # (e.g. "Running"), otherwise the identifier split into words ("Program running")
    "text": TextFormat.TEXT,
}
TextInfo = freemdu_ns.enum("TextInfo", is_class=True)
INFO = {
    # Software ID of the connected device, e.g. 457
    "software_id": TextInfo.SOFTWARE_ID,
    # Device kind, e.g. Washing Machine
    "device_kind": TextInfo.DEVICE_KIND,
    # Outcome of the last poll, e.g. "Ok" or "No answer from the device"
    "status": TextInfo.STATUS,
    # Outcome of the last adapter test (see the button platform)
    "adapter_test": TextInfo.ADAPTER_TEST,
}

CONFIG_SCHEMA = cv.All(
    text_sensor.text_sensor_schema()
    .extend(FREEMDU_CHILD_SCHEMA)
    .extend(
        {
            cv.Exclusive(CONF_PROPERTY, "source"): PROPERTY_ID,
            cv.Exclusive(CONF_INFO, "source"): cv.enum(INFO, lower=True),
            cv.Optional(CONF_FORMAT, default="code"): cv.enum(FORMATS, lower=True),
        }
    ),
    cv.has_exactly_one_key(CONF_PROPERTY, CONF_INFO),
)


async def to_code(config):
    parent = await cg.get_variable(config[CONF_FREEMDU_ID])
    var = await text_sensor.new_text_sensor(config)
    if CONF_PROPERTY in config:
        cg.add(parent.add_text_sensor(config[CONF_PROPERTY], var, config[CONF_FORMAT]))
    else:
        cg.add(parent.set_info_text_sensor(config[CONF_INFO], var))
