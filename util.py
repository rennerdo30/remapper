import json
import evdev

import inputdevice

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
    presets.append({"name": "Xbox Gamepad", "vendor": 1118,
                    "product": 654, "version": 272})

    return presets


def translate(value, leftMin, leftMax, rightMin, rightMax):
    leftSpan = leftMax - leftMin
    rightSpan = rightMax - rightMin
    valueScaled = float(value - leftMin) / float(leftSpan)
    return rightMin + (valueScaled * rightSpan)


def select_evdev_via_console():
    print("The following devices are available:")
    devices = [evdev.InputDevice(path) for path in evdev.list_devices()]
    counter = 0
    for device in devices:
        input_device = inputdevice.InputDevice(device)
        print("[", str(counter), "]", input_device.info_string())
        counter = counter + 1

    print("Select the device to remap by entering the number in front of the desired entry.")
    entered_idx = input()

    valid_input = False
    device = None
    while not valid_input:
        if entered_idx.isnumeric():
            input_idx = int(entered_idx)
            if input_idx < 0 or input_idx > len(devices) - 1:
                valid_input = False
                print("Input was not valid. Please try again!")
                entered_idx = input()
            else:
                valid_input = True
                entered_idx = input_idx

                device = inputdevice.InputDevice(devices[entered_idx])
                print("Selected device:", device.info_string())
                print("Do you want to continue? [y/n]")
                confirm = input()
                if confirm == 'y':
                    break
                else:
                    print("Enter new index:")
                    entered_idx = input()
                    valid_input = False
                    continue
    return device
