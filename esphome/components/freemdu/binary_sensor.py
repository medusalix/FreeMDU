import esphome.codegen as cg
from esphome.components import binary_sensor
import esphome.config_validation as cv
from esphome.const import DEVICE_CLASS_CONNECTIVITY

from . import CONF_FREEMDU_ID, CONF_PROPERTY, FREEMDU_CHILD_SCHEMA, PROPERTY_ID

DEPENDENCIES = ["freemdu"]

CONF_CONNECTED = "connected"
CONF_ECHO = "echo"

CONFIG_SCHEMA = cv.All(
    binary_sensor.binary_sensor_schema()
    .extend(FREEMDU_CHILD_SCHEMA)
    .extend(
        {
            cv.Exclusive(CONF_PROPERTY, "source"): PROPERTY_ID,
            # On while the device answers; tells a switched-off machine from a broken adapter
            cv.Exclusive(CONF_CONNECTED, "source"): cv.boolean,
            # On while the adapter test sees its own transmission come back; for an LED
            cv.Exclusive(CONF_ECHO, "source"): cv.boolean,
        }
    ),
    cv.has_exactly_one_key(CONF_PROPERTY, CONF_CONNECTED, CONF_ECHO),
)


async def to_code(config):
    parent = await cg.get_variable(config[CONF_FREEMDU_ID])
    if CONF_PROPERTY in config:
        var = await binary_sensor.new_binary_sensor(config)
        cg.add(parent.add_binary_sensor(config[CONF_PROPERTY], var))
    elif CONF_ECHO in config:
        var = await binary_sensor.new_binary_sensor(config)
        cg.add(parent.set_echo_binary_sensor(var))
    else:
        if "device_class" not in config:
            config["device_class"] = DEVICE_CLASS_CONNECTIVITY
        var = await binary_sensor.new_binary_sensor(config)
        cg.add(parent.set_connected_binary_sensor(var))
