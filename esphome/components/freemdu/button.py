import esphome.codegen as cg
from esphome.components import button
import esphome.config_validation as cv
from esphome.const import ENTITY_CATEGORY_DIAGNOSTIC

from . import CONF_FREEMDU_ID, FREEMDU_CHILD_SCHEMA, freemdu_ns

DEPENDENCIES = ["freemdu"]

CONF_ADAPTER_TEST = "adapter_test"

AdapterTestButton = freemdu_ns.class_("AdapterTestButton", button.Button)

CONFIG_SCHEMA = (
    button.button_schema(
        AdapterTestButton,
        entity_category=ENTITY_CATEGORY_DIAGNOSTIC,
        icon="mdi:test-tube",
    )
    .extend(FREEMDU_CHILD_SCHEMA)
    .extend(
        {
            # Checks the adapter on its own, without the appliance: a minute of echo checks
            cv.Required(CONF_ADAPTER_TEST): cv.boolean,
        }
    )
)


async def to_code(config):
    var = await button.new_button(config)
    await cg.register_parented(var, config[CONF_FREEMDU_ID])
