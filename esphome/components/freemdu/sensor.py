import esphome.codegen as cg
from esphome.components import sensor
import esphome.config_validation as cv

from . import CONF_FREEMDU_ID, CONF_PROPERTY, FREEMDU_CHILD_SCHEMA, PROPERTY_ID

DEPENDENCIES = ["freemdu"]

CONF_VALUE = "value"
# Which half of a current/target pair to publish; ignored for plain numbers
VALUES = {"current": False, "target": True}

CONFIG_SCHEMA = (
    sensor.sensor_schema()
    .extend(FREEMDU_CHILD_SCHEMA)
    .extend(
        {
            cv.Required(CONF_PROPERTY): PROPERTY_ID,
            cv.Optional(CONF_VALUE, default="current"): cv.one_of(*VALUES, lower=True),
        }
    )
)


async def to_code(config):
    parent = await cg.get_variable(config[CONF_FREEMDU_ID])
    var = await sensor.new_sensor(config)
    cg.add(parent.add_sensor(config[CONF_PROPERTY], var, VALUES[config[CONF_VALUE]]))
