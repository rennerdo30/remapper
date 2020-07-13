import json
import evdev


def str2bool(v):
    return v.lower() in ("y", "yes", "true", "t", "1")


def value_to_ecode(value):
    if value is None:
        return None
    elif str(value).isnumeric():
        value = int(value)
        for key, code in evdev.ecodes.ecodes.items():
            if value == code:
                return code
    else:
        if value.upper() in evdev.ecodes.ecodes:
            return evdev.ecodes.ecodes[value.upper()]
    return None


def uinput_presets():
    presets = []
    presets.append({"name": "Xbox Gamepad", "vendor": 1118, "product": 654, "version": 272})

    return presets

def translate(value, leftMin, leftMax, rightMin, rightMax):
    leftSpan = leftMax - leftMin
    rightSpan = rightMax - rightMin
    valueScaled = float(value - leftMin) / float(leftSpan)
    return rightMin + (valueScaled * rightSpan)